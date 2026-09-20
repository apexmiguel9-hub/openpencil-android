use std::io::{Cursor, Read};

use flate2::read::DeflateDecoder;

use crate::project::{
    is_html_path, is_ignored_metadata_path, normalize_relative_path, select_html_entry_index,
    strip_common_root_paths, virtual_resource_key, virtual_url, HtmlProjectError,
    MAX_PROJECT_FILES,
};
use crate::project_zip::{find_zip_eocd, read_u16, read_u32, MAX_ZIP_ENTRIES};
use crate::zip_encoding::{crc32, decode_zip_name};

pub const HTML_ZIP_EOCD_MIN_BYTES: usize = 22;
pub const HTML_ZIP_TAIL_BYTES: usize = HTML_ZIP_EOCD_MIN_BYTES + u16::MAX as usize;
pub const HTML_ZIP_LOCAL_HEADER_FIXED_BYTES: usize = 30;

const CENTRAL_HEADER_FIXED_BYTES: usize = 46;
const CENTRAL_HEADER_SIGNATURE: &[u8; 4] = b"PK\x01\x02";
const LOCAL_HEADER_SIGNATURE: &[u8; 4] = b"PK\x03\x04";

/// The byte range that a range-capable host must fetch for the central directory.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HtmlZipDirectoryLocator {
    pub archive_len: u64,
    pub central_directory_offset: u64,
    pub central_directory_size: u64,
    pub entry_count: usize,
    pub eocd_offset: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HtmlZipCompressionMethod {
    Stored,
    Deflated,
}

impl HtmlZipCompressionMethod {
    fn from_raw(path: &str, value: u16) -> Result<Self, HtmlProjectError> {
        match value {
            0 => Ok(Self::Stored),
            8 => Ok(Self::Deflated),
            _ => Err(HtmlProjectError::UnsupportedZipEntry {
                path: path.to_string(),
                reason: format!(
                    "compression method {value} is not supported; use stored or deflate"
                ),
            }),
        }
    }

    fn raw(self) -> u16 {
        match self {
            Self::Stored => 0,
            Self::Deflated => 8,
        }
    }
}

/// Metadata required to fetch and decode one ZIP entry without loading the archive.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HtmlZipRangeEntry {
    pub relative_path: String,
    pub local_header_offset: u64,
    pub compressed_size: u64,
    pub uncompressed_size: u64,
    pub compression_method: HtmlZipCompressionMethod,
    pub crc32: u32,
    raw_name: Vec<u8>,
    flags: u16,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HtmlZipRangeManifest {
    pub archive_len: u64,
    pub central_directory_offset: u64,
    pub entries: Vec<HtmlZipRangeEntry>,
    pub html_entry_index: usize,
    pub html_entry_count: usize,
}

impl HtmlZipRangeManifest {
    pub fn html_entry(&self) -> &HtmlZipRangeEntry {
        &self.entries[self.html_entry_index]
    }

    /// Resolves a virtual project URL to an entry index after removing its query and fragment.
    pub fn entry_for_resource_url(&self, requested: &str) -> Option<usize> {
        let key = virtual_resource_key(requested)?;
        self.entries
            .iter()
            .position(|entry| virtual_url(&entry.relative_path) == key)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HtmlZipByteRange {
    pub offset: u64,
    pub length: u64,
}

/// Returns the virtual URL used as the imported entry's base URL.
pub fn html_zip_project_url(relative_path: &str) -> String {
    virtual_url(relative_path)
}

/// Locates an EOCD record in an exact suffix of an archive.
///
/// Hosts should normally pass the final `min(archive_len, HTML_ZIP_TAIL_BYTES)`
/// bytes. A smaller suffix is accepted when it still contains the complete EOCD.
pub fn locate_html_zip_directory(
    archive_len: u64,
    tail: &[u8],
) -> Result<HtmlZipDirectoryLocator, HtmlProjectError> {
    if archive_len < HTML_ZIP_EOCD_MIN_BYTES as u64
        || tail.len() < HTML_ZIP_EOCD_MIN_BYTES
        || tail.len() as u64 > archive_len
    {
        return Err(invalid_zip("end-of-central-directory record is missing"));
    }
    let tail_offset = archive_len - tail.len() as u64;
    let Some(eocd) = find_zip_eocd(tail) else {
        return Err(invalid_zip(
            "end-of-central-directory record is missing or malformed",
        ));
    };
    if eocd.disk != 0 || eocd.directory_disk != 0 || eocd.disk_entries != eocd.total_entries {
        return Err(invalid_zip("multi-disk ZIP archives are not supported"));
    }
    if eocd.total_entries == u16::MAX
        || eocd.directory_size == u32::MAX
        || eocd.directory_offset == u32::MAX
    {
        return Err(invalid_zip(
            "ZIP64 archives are not supported for HTML import",
        ));
    }
    if eocd.total_entries as usize > MAX_ZIP_ENTRIES {
        return Err(HtmlProjectError::TooManyFiles {
            count: eocd.total_entries as usize,
            limit: MAX_ZIP_ENTRIES,
        });
    }
    let eocd_offset = tail_offset + eocd.local_offset as u64;
    let directory_end = (eocd.directory_offset as u64)
        .checked_add(eocd.directory_size as u64)
        .ok_or_else(|| invalid_zip("central-directory bounds overflow"))?;
    if directory_end > eocd_offset {
        return Err(invalid_zip("central-directory bounds are invalid"));
    }
    Ok(HtmlZipDirectoryLocator {
        archive_len,
        central_directory_offset: eocd.directory_offset as u64,
        central_directory_size: eocd.directory_size as u64,
        entry_count: eocd.total_entries as usize,
        eocd_offset,
    })
}

/// Parses the exact central-directory range described by `locator`.
pub fn parse_html_zip_central_directory(
    locator: &HtmlZipDirectoryLocator,
    directory: &[u8],
) -> Result<HtmlZipRangeManifest, HtmlProjectError> {
    if directory.len() as u64 != locator.central_directory_size {
        return Err(invalid_zip(
            "central-directory slice length does not match the EOCD record",
        ));
    }
    let mut entries = Vec::with_capacity(locator.entry_count.min(MAX_PROJECT_FILES));
    let mut seen = std::collections::HashSet::new();
    let mut offset = 0usize;
    for _ in 0..locator.entry_count {
        let fixed_end = offset
            .checked_add(CENTRAL_HEADER_FIXED_BYTES)
            .ok_or_else(|| invalid_zip("central-directory entry bounds overflow"))?;
        let header = directory
            .get(offset..fixed_end)
            .ok_or_else(|| invalid_zip("central-directory entry is truncated"))?;
        if !header.starts_with(CENTRAL_HEADER_SIGNATURE) {
            return Err(invalid_zip("central-directory entry signature is invalid"));
        }
        let made_by = required_u16(header, 4, "central-directory entry")?;
        let flags = required_u16(header, 8, "central-directory entry")?;
        let method = required_u16(header, 10, "central-directory entry")?;
        let crc32 = required_u32(header, 16, "central-directory entry")?;
        let compressed_size = required_u32(header, 20, "central-directory entry")?;
        let uncompressed_size = required_u32(header, 24, "central-directory entry")?;
        let name_len = required_u16(header, 28, "central-directory entry")? as usize;
        let extra_len = required_u16(header, 30, "central-directory entry")? as usize;
        let comment_len = required_u16(header, 32, "central-directory entry")? as usize;
        let disk_start = required_u16(header, 34, "central-directory entry")?;
        let external_attributes = required_u32(header, 38, "central-directory entry")?;
        let local_header_offset = required_u32(header, 42, "central-directory entry")?;
        if compressed_size == u32::MAX
            || uncompressed_size == u32::MAX
            || local_header_offset == u32::MAX
        {
            return Err(invalid_zip(
                "ZIP64 entries are not supported for HTML import",
            ));
        }
        if disk_start != 0 {
            return Err(invalid_zip("multi-disk ZIP archives are not supported"));
        }
        let record_len = CENTRAL_HEADER_FIXED_BYTES
            .checked_add(name_len)
            .and_then(|value| value.checked_add(extra_len))
            .and_then(|value| value.checked_add(comment_len))
            .ok_or_else(|| invalid_zip("central-directory entry bounds overflow"))?;
        let record_end = offset
            .checked_add(record_len)
            .ok_or_else(|| invalid_zip("central-directory entry bounds overflow"))?;
        let name_end = fixed_end
            .checked_add(name_len)
            .ok_or_else(|| invalid_zip("central-directory entry bounds overflow"))?;
        let extra_end = name_end
            .checked_add(extra_len)
            .ok_or_else(|| invalid_zip("central-directory entry bounds overflow"))?;
        let name = directory
            .get(fixed_end..name_end)
            .ok_or_else(|| invalid_zip("central-directory file name is truncated"))?;
        let extra = directory
            .get(name_end..extra_end)
            .ok_or_else(|| invalid_zip("central-directory extra field is truncated"))?;
        directory
            .get(offset..record_end)
            .ok_or_else(|| invalid_zip("central-directory entry is truncated"))?;
        offset = record_end;

        let original_path = decode_zip_name(name, extra, flags)?;
        let path = normalize_relative_path(&original_path)?;
        let is_dir = name.ends_with(b"/")
            || name.ends_with(b"\\")
            || original_path.ends_with('/')
            || original_path.ends_with('\\');
        let unix_kind = ((external_attributes >> 16) as u16) & 0o170000;
        let is_unix = made_by >> 8 == 3;
        if is_unix && unix_kind == 0o120000 {
            return Err(HtmlProjectError::UnsupportedZipEntry {
                path,
                reason: "symbolic links are not imported".to_string(),
            });
        }
        if is_dir || is_ignored_metadata_path(&path) {
            continue;
        }
        if entries.len() >= MAX_PROJECT_FILES {
            return Err(HtmlProjectError::TooManyFiles {
                count: entries.len() + 1,
                limit: MAX_PROJECT_FILES,
            });
        }
        if is_unix && unix_kind != 0 && unix_kind != 0o100000 {
            return Err(HtmlProjectError::UnsupportedZipEntry {
                path,
                reason: "only regular files are imported".to_string(),
            });
        }
        if flags & 1 != 0 {
            return Err(HtmlProjectError::UnsupportedZipEntry {
                path,
                reason: "encrypted entries are not supported".to_string(),
            });
        }
        let compression_method = HtmlZipCompressionMethod::from_raw(&path, method)?;
        if compression_method == HtmlZipCompressionMethod::Stored
            && compressed_size != uncompressed_size
        {
            return Err(invalid_zip(
                "stored entry compressed and uncompressed sizes do not match",
            ));
        }
        if (local_header_offset as u64)
            .checked_add(HTML_ZIP_LOCAL_HEADER_FIXED_BYTES as u64)
            .is_none_or(|end| end > locator.central_directory_offset)
        {
            return Err(invalid_zip("local-file-header bounds are invalid"));
        }
        if !seen.insert(path.clone()) {
            return Err(HtmlProjectError::DuplicatePath(path));
        }
        entries.push(HtmlZipRangeEntry {
            relative_path: path,
            local_header_offset: local_header_offset as u64,
            compressed_size: compressed_size as u64,
            uncompressed_size: uncompressed_size as u64,
            compression_method,
            crc32,
            raw_name: name.to_vec(),
            flags,
        });
    }
    if offset != directory.len() {
        return Err(invalid_zip(
            "central-directory contains unexpected trailing bytes",
        ));
    }
    if entries.is_empty() {
        return Err(HtmlProjectError::EmptyProject);
    }
    let mut paths: Vec<String> = entries
        .iter()
        .map(|entry| entry.relative_path.clone())
        .collect();
    strip_common_root_paths(&mut paths);
    let mut stripped_seen = std::collections::HashSet::new();
    for (entry, path) in entries.iter_mut().zip(paths) {
        if !stripped_seen.insert(path.clone()) {
            return Err(HtmlProjectError::DuplicatePath(path));
        }
        entry.relative_path = path;
    }
    let paths: Vec<String> = entries
        .iter()
        .map(|entry| entry.relative_path.clone())
        .collect();
    let html_entry_index = select_html_entry_index(&paths)?;
    let html_entry_count = paths.iter().filter(|path| is_html_path(path)).count();
    Ok(HtmlZipRangeManifest {
        archive_len: locator.archive_len,
        central_directory_offset: locator.central_directory_offset,
        entries,
        html_entry_index,
        html_entry_count,
    })
}

/// Computes the compressed-data range from the fixed 30-byte local header.
pub fn html_zip_entry_data_range(
    manifest: &HtmlZipRangeManifest,
    entry: &HtmlZipRangeEntry,
    local_header: &[u8],
) -> Result<HtmlZipByteRange, HtmlProjectError> {
    let parsed = parse_local_header_prefix(entry, local_header)?;
    validate_local_name_if_present(entry, local_header, &parsed)?;
    let offset = entry
        .local_header_offset
        .checked_add(parsed.header_len)
        .ok_or_else(|| invalid_zip("local-file-header bounds overflow"))?;
    let end = offset
        .checked_add(entry.compressed_size)
        .ok_or_else(|| invalid_zip("compressed entry bounds overflow"))?;
    if end > manifest.central_directory_offset || end > manifest.archive_len {
        return Err(invalid_zip("compressed entry bounds are invalid"));
    }
    Ok(HtmlZipByteRange {
        offset,
        length: entry.compressed_size,
    })
}

/// Decodes and CRC-checks one entry fetched with `Blob.slice` or an equivalent API.
///
/// `local_header` may contain only the fixed 30-byte prefix. When it also contains
/// the file name, that name is checked against the central-directory metadata.
pub fn decode_html_zip_entry(
    entry: &HtmlZipRangeEntry,
    local_header: &[u8],
    compressed: &[u8],
) -> Result<Vec<u8>, HtmlProjectError> {
    let parsed = parse_local_header_prefix(entry, local_header)?;
    validate_local_name_if_present(entry, local_header, &parsed)?;
    if compressed.len() as u64 != entry.compressed_size {
        return Err(invalid_zip(
            "compressed entry slice length does not match its directory metadata",
        ));
    }
    let expected_len = usize::try_from(entry.uncompressed_size)
        .map_err(|_| invalid_zip("uncompressed entry is too large for this platform"))?;
    let mut decoded = Vec::new();
    decoded
        .try_reserve_exact(expected_len)
        .map_err(|_| invalid_zip("unable to allocate the uncompressed entry"))?;
    match entry.compression_method {
        HtmlZipCompressionMethod::Stored => decoded.extend_from_slice(compressed),
        HtmlZipCompressionMethod::Deflated => {
            let decoder = DeflateDecoder::new(Cursor::new(compressed));
            let output_limit = entry
                .uncompressed_size
                .checked_add(1)
                .ok_or_else(|| invalid_zip("uncompressed entry size overflows"))?;
            let mut limited = decoder.take(output_limit);
            limited
                .read_to_end(&mut decoded)
                .map_err(|error| invalid_zip(&format!("deflate decoding failed: {error}")))?;
            let decoder = limited.into_inner();
            if decoder.total_in() != compressed.len() as u64 {
                return Err(invalid_zip("deflate stream contains trailing data"));
            }
        }
    };
    if decoded.len() != expected_len {
        return Err(invalid_zip(
            "uncompressed entry size does not match its directory metadata",
        ));
    }
    if crc32(&decoded) != entry.crc32 {
        return Err(invalid_zip("entry CRC-32 does not match"));
    }
    Ok(decoded)
}

/// Returns the complete local-file-header length from its fixed 30-byte prefix.
///
/// A range-capable host can fetch this many bytes and pass them to
/// [`validate_html_zip_local_header`] before fetching the compressed payload.
pub fn html_zip_local_header_length(
    entry: &HtmlZipRangeEntry,
    local_header_prefix: &[u8],
) -> Result<u64, HtmlProjectError> {
    Ok(parse_local_header_prefix(entry, local_header_prefix)?.header_len)
}

/// Validates a complete local-file header, including its raw file name.
///
/// The supplied slice may contain bytes after the header, but it must contain
/// at least the complete name and extra-field region declared by the prefix.
pub fn validate_html_zip_local_header(
    entry: &HtmlZipRangeEntry,
    local_header: &[u8],
) -> Result<(), HtmlProjectError> {
    let parsed = parse_local_header_prefix(entry, local_header)?;
    let header_len = usize::try_from(parsed.header_len)
        .map_err(|_| invalid_zip("local-file-header length is too large for this platform"))?;
    if local_header.len() < header_len {
        return Err(invalid_zip("complete local-file header is truncated"));
    }
    validate_local_name(entry, local_header, &parsed)
}

struct ParsedLocalHeader {
    header_len: u64,
    name_len: usize,
}

fn parse_local_header_prefix(
    entry: &HtmlZipRangeEntry,
    bytes: &[u8],
) -> Result<ParsedLocalHeader, HtmlProjectError> {
    let header = bytes
        .get(..HTML_ZIP_LOCAL_HEADER_FIXED_BYTES)
        .ok_or_else(|| invalid_zip("local-file-header prefix is truncated"))?;
    if !header.starts_with(LOCAL_HEADER_SIGNATURE) {
        return Err(invalid_zip("local-file-header signature is invalid"));
    }
    let flags = required_u16(header, 6, "local-file header")?;
    let method = required_u16(header, 8, "local-file header")?;
    let local_crc = required_u32(header, 14, "local-file header")?;
    let local_compressed = required_u32(header, 18, "local-file header")?;
    let local_uncompressed = required_u32(header, 22, "local-file header")?;
    let name_len = required_u16(header, 26, "local-file header")? as usize;
    let extra_len = required_u16(header, 28, "local-file header")? as usize;
    if name_len != entry.raw_name.len() {
        return Err(invalid_zip(
            "local and central entry file-name lengths do not match",
        ));
    }
    if flags & 1 != 0 {
        return Err(HtmlProjectError::UnsupportedZipEntry {
            path: entry.relative_path.clone(),
            reason: "encrypted entries are not supported".to_string(),
        });
    }
    if flags != entry.flags {
        return Err(invalid_zip("local and central entry flags do not match"));
    }
    if method != entry.compression_method.raw() {
        return Err(invalid_zip(
            "local and central compression methods do not match",
        ));
    }
    let uses_descriptor = flags & (1 << 3) != 0;
    if !uses_descriptor
        && (local_crc != entry.crc32
            || local_compressed as u64 != entry.compressed_size
            || local_uncompressed as u64 != entry.uncompressed_size)
    {
        return Err(invalid_zip("local and central entry metadata do not match"));
    }
    let header_len = HTML_ZIP_LOCAL_HEADER_FIXED_BYTES
        .checked_add(name_len)
        .and_then(|value| value.checked_add(extra_len))
        .ok_or_else(|| invalid_zip("local-file-header bounds overflow"))?;
    Ok(ParsedLocalHeader {
        header_len: header_len as u64,
        name_len,
    })
}

fn validate_local_name_if_present(
    entry: &HtmlZipRangeEntry,
    bytes: &[u8],
    parsed: &ParsedLocalHeader,
) -> Result<(), HtmlProjectError> {
    let name_end = HTML_ZIP_LOCAL_HEADER_FIXED_BYTES
        .checked_add(parsed.name_len)
        .ok_or_else(|| invalid_zip("local-file-header bounds overflow"))?;
    if bytes.len() >= name_end {
        validate_local_name(entry, bytes, parsed)?;
    }
    Ok(())
}

fn validate_local_name(
    entry: &HtmlZipRangeEntry,
    bytes: &[u8],
    parsed: &ParsedLocalHeader,
) -> Result<(), HtmlProjectError> {
    let name_end = HTML_ZIP_LOCAL_HEADER_FIXED_BYTES
        .checked_add(parsed.name_len)
        .ok_or_else(|| invalid_zip("local-file-header bounds overflow"))?;
    let local_name = bytes
        .get(HTML_ZIP_LOCAL_HEADER_FIXED_BYTES..name_end)
        .ok_or_else(|| invalid_zip("local-file-header file name is truncated"))?;
    if local_name != entry.raw_name {
        return Err(invalid_zip(
            "local and central entry file names do not match",
        ));
    }
    Ok(())
}

fn invalid_zip(detail: &str) -> HtmlProjectError {
    HtmlProjectError::InvalidZip(detail.to_string())
}

fn required_u16(bytes: &[u8], offset: usize, context: &str) -> Result<u16, HtmlProjectError> {
    read_u16(bytes, offset).ok_or_else(|| invalid_zip(&format!("{context} is truncated")))
}

fn required_u32(bytes: &[u8], offset: usize, context: &str) -> Result<u32, HtmlProjectError> {
    read_u32(bytes, offset).ok_or_else(|| invalid_zip(&format!("{context} is truncated")))
}

#[cfg(test)]
mod tests {
    use zip::CompressionMethod;

    use super::*;
    use crate::project_zip::test_archive as archive;

    fn parse(bytes: &[u8]) -> (HtmlZipDirectoryLocator, HtmlZipRangeManifest) {
        let tail_start = bytes.len().saturating_sub(HTML_ZIP_TAIL_BYTES);
        let locator = locate_html_zip_directory(bytes.len() as u64, &bytes[tail_start..]).unwrap();
        let start = locator.central_directory_offset as usize;
        let end = start + locator.central_directory_size as usize;
        let manifest = parse_html_zip_central_directory(&locator, &bytes[start..end]).unwrap();
        (locator, manifest)
    }

    fn decode(bytes: &[u8], manifest: &HtmlZipRangeManifest, index: usize) -> Vec<u8> {
        let entry = &manifest.entries[index];
        let header_start = entry.local_header_offset as usize;
        let header = &bytes[header_start..header_start + HTML_ZIP_LOCAL_HEADER_FIXED_BYTES];
        let range = html_zip_entry_data_range(manifest, entry, header).unwrap();
        let start = range.offset as usize;
        decode_html_zip_entry(entry, header, &bytes[start..start + range.length as usize]).unwrap()
    }

    #[test]
    fn locates_selects_and_decodes_stored_and_deflated_entries() {
        let bytes = archive(&[
            ("bundle/readme.html", b"other", CompressionMethod::Stored),
            (
                "bundle/index.html",
                b"<link href='assets/site.css'>",
                CompressionMethod::Deflated,
            ),
            (
                "bundle/assets/site.css",
                b"body{color:red}",
                CompressionMethod::Stored,
            ),
        ]);
        let (locator, manifest) = parse(&bytes);
        assert_eq!(locator.archive_len, bytes.len() as u64);
        assert_eq!(manifest.html_entry().relative_path, "index.html");
        assert_eq!(manifest.html_entry_count, 2);
        assert_eq!(
            manifest
                .entry_for_resource_url("https://openpencil.local/assets/site.css?cache=1#ignored"),
            manifest
                .entries
                .iter()
                .position(|entry| entry.relative_path == "assets/site.css")
        );
        assert_eq!(
            html_zip_project_url("assets/my image.png"),
            "https://openpencil.local/assets/my%20image.png"
        );
        assert_eq!(
            decode(&bytes, &manifest, manifest.html_entry_index),
            b"<link href='assets/site.css'>"
        );
        let css = manifest
            .entries
            .iter()
            .position(|entry| entry.relative_path == "assets/site.css")
            .unwrap();
        assert_eq!(decode(&bytes, &manifest, css), b"body{color:red}");
    }

    #[test]
    fn rejects_bad_tail_directory_and_unsafe_metadata() {
        let source = archive(&[("index.html", b"ok", CompressionMethod::Stored)]);
        assert!(
            locate_html_zip_directory(source.len() as u64, &source[source.len() - 10..]).is_err()
        );

        let (locator, _) = parse(&source);
        let start = locator.central_directory_offset as usize;
        let end = start + locator.central_directory_size as usize;
        assert!(parse_html_zip_central_directory(&locator, &source[start..end - 1]).is_err());

        let duplicate = archive(&[
            ("index.html", b"a", CompressionMethod::Stored),
            ("./index.html", b"b", CompressionMethod::Stored),
        ]);
        let tail_start = duplicate.len().saturating_sub(HTML_ZIP_TAIL_BYTES);
        let locator =
            locate_html_zip_directory(duplicate.len() as u64, &duplicate[tail_start..]).unwrap();
        let start = locator.central_directory_offset as usize;
        let end = start + locator.central_directory_size as usize;
        assert!(matches!(
            parse_html_zip_central_directory(&locator, &duplicate[start..end]),
            Err(HtmlProjectError::DuplicatePath(_))
        ));

        let mut encrypted = source.clone();
        let (_, clean) = parse(&encrypted);
        let central = locator_for(&encrypted).central_directory_offset as usize;
        encrypted[central + 8] |= 1;
        let locator = locator_for(&encrypted);
        let end = central + locator.central_directory_size as usize;
        assert!(matches!(
            parse_html_zip_central_directory(&locator, &encrypted[central..end]),
            Err(HtmlProjectError::UnsupportedZipEntry { reason, .. }) if reason.contains("encrypted")
        ));
        assert_eq!(clean.entries.len(), 1);

        let mut fifo = source.clone();
        let central = locator_for(&fifo).central_directory_offset as usize;
        fifo[central + 5] = 3;
        fifo[central + 38..central + 42].copy_from_slice(&(0o010644u32 << 16).to_le_bytes());
        let locator = locator_for(&fifo);
        let end = central + locator.central_directory_size as usize;
        assert!(matches!(
            parse_html_zip_central_directory(&locator, &fifo[central..end]),
            Err(HtmlProjectError::UnsupportedZipEntry { reason, .. }) if reason.contains("regular")
        ));

        let mut bad_stored_size = source;
        let central = locator_for(&bad_stored_size).central_directory_offset as usize;
        bad_stored_size[central + 24..central + 28].copy_from_slice(&3u32.to_le_bytes());
        let locator = locator_for(&bad_stored_size);
        let end = central + locator.central_directory_size as usize;
        assert!(
            parse_html_zip_central_directory(&locator, &bad_stored_size[central..end]).is_err()
        );

        assert_eq!(decode_zip_name(b"caf\x82.png", &[], 0).unwrap(), "café.png");
    }

    #[test]
    fn validates_local_header_lengths_crc_and_compressed_data() {
        let mut bytes = archive(&[(
            "index.html",
            b"a reasonably compressible page page page",
            CompressionMethod::Deflated,
        )]);
        let (_, manifest) = parse(&bytes);
        let entry = manifest.html_entry();
        let start = entry.local_header_offset as usize;
        let header = bytes[start..start + HTML_ZIP_LOCAL_HEADER_FIXED_BYTES].to_vec();
        let full_header_len = html_zip_local_header_length(entry, &header).unwrap() as usize;
        let full_header = bytes[start..start + full_header_len].to_vec();
        validate_html_zip_local_header(entry, &full_header).unwrap();
        assert!(
            validate_html_zip_local_header(entry, &full_header[..full_header.len() - 1]).is_err()
        );
        let mut wrong_full_header = full_header;
        wrong_full_header[HTML_ZIP_LOCAL_HEADER_FIXED_BYTES] ^= 1;
        assert!(validate_html_zip_local_header(entry, &wrong_full_header).is_err());
        let range = html_zip_entry_data_range(&manifest, entry, &header).unwrap();
        let compressed_start = range.offset as usize;
        let compressed_end = compressed_start + range.length as usize;
        assert_eq!(
            decode_html_zip_entry(entry, &header, &bytes[compressed_start..compressed_end])
                .unwrap(),
            b"a reasonably compressible page page page"
        );
        bytes[compressed_start] ^= 0x40;
        assert!(
            decode_html_zip_entry(entry, &header, &bytes[compressed_start..compressed_end])
                .is_err()
        );

        let mut wrong_header = header;
        wrong_header[8..10].copy_from_slice(&0u16.to_le_bytes());
        assert!(html_zip_entry_data_range(&manifest, entry, &wrong_header).is_err());

        let mut wrong_name_len = wrong_header;
        wrong_name_len[8..10].copy_from_slice(&8u16.to_le_bytes());
        wrong_name_len[26..28].copy_from_slice(&1u16.to_le_bytes());
        assert!(html_zip_entry_data_range(&manifest, entry, &wrong_name_len).is_err());
    }

    fn locator_for(bytes: &[u8]) -> HtmlZipDirectoryLocator {
        let tail_start = bytes.len().saturating_sub(HTML_ZIP_TAIL_BYTES);
        locate_html_zip_directory(bytes.len() as u64, &bytes[tail_start..]).unwrap()
    }
}
