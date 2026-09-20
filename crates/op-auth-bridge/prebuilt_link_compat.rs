//! Deterministic compatibility staging for C-facing Rust static libraries.
//!
//! A Rust `staticlib` contains its own Rust runtime because it is normally
//! linked by a C application. When that archive is linked back into a Rust
//! executable, newer ELF/COFF linkers reject the archive's
//! `rust_eh_personality` alongside the host toolchain's definition. The
//! original, provenance-checked archive stays untouched. Only the private
//! Cargo `OUT_DIR` copy is rewritten, using an equal-length private symbol
//! name so archive offsets, object symbol indexes, and relocations remain
//! unchanged.

use std::fmt;
use std::fs;
use std::io;
use std::path::Path;

use sha2::{Digest, Sha256};

const ARCHIVE_MAGIC: &[u8] = b"!<arch>\n";
const HOST_PERSONALITY: &[u8] = b"rust_eh_personality";
// Keep the alias adjacent to the original name. The MSVC archive's second
// linker member is lexically sorted and some linkers binary-search it; moving
// the entry to an `op_auth_*` prefix would invalidate that index.
const PRIVATE_PERSONALITY: &[u8] = b"rust_eh_personalitx";
const _: [(); HOST_PERSONALITY.len()] = [(); PRIVATE_PERSONALITY.len()];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LinkCompatReport {
    pub renamed_occurrences: usize,
    pub existing_private_occurrences: usize,
}

#[derive(Debug)]
pub enum LinkCompatError {
    Io {
        operation: &'static str,
        source: io::Error,
    },
    InvalidArchive(&'static str),
    ConflictingPersonalitySymbols,
    SourceDigestMismatch,
    SourceEqualsDestination,
}

impl fmt::Display for LinkCompatError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { operation, source } => write!(formatter, "{operation}: {source}"),
            Self::InvalidArchive(reason) => {
                write!(formatter, "static archive is malformed: {reason}")
            }
            Self::ConflictingPersonalitySymbols => formatter
                .write_str("archive contains both host and private Rust personality symbol names"),
            Self::SourceDigestMismatch => {
                formatter.write_str("source archive changed after provenance validation")
            }
            Self::SourceEqualsDestination => {
                formatter.write_str("compatibility staging must not modify the source archive")
            }
        }
    }
}

impl std::error::Error for LinkCompatError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

pub fn stage_archive_for_rust_host(
    source: &Path,
    destination: &Path,
    expected_sha256: Option<[u8; 32]>,
) -> Result<LinkCompatReport, LinkCompatError> {
    if source == destination {
        return Err(LinkCompatError::SourceEqualsDestination);
    }

    let mut archive = fs::read(source).map_err(|source| LinkCompatError::Io {
        operation: "failed to read source archive",
        source,
    })?;
    if expected_sha256
        .is_some_and(|expected| <[u8; 32]>::from(Sha256::digest(&archive)) != expected)
    {
        return Err(LinkCompatError::SourceDigestMismatch);
    }
    validate_archive(&archive)?;

    let host_count = count_occurrences(&archive, HOST_PERSONALITY);
    let private_count = count_occurrences(&archive, PRIVATE_PERSONALITY);
    if host_count != 0 && private_count != 0 {
        return Err(LinkCompatError::ConflictingPersonalitySymbols);
    }

    if host_count != 0 {
        replace_equal_length(&mut archive, HOST_PERSONALITY, PRIVATE_PERSONALITY);
    }
    if count_occurrences(&archive, HOST_PERSONALITY) != 0 {
        return Err(LinkCompatError::InvalidArchive(
            "host Rust personality symbol remained after staging",
        ));
    }
    validate_archive(&archive)?;

    let parent = destination.parent().ok_or(LinkCompatError::InvalidArchive(
        "staged archive path has no parent",
    ))?;
    fs::create_dir_all(parent).map_err(|source| LinkCompatError::Io {
        operation: "failed to create archive staging directory",
        source,
    })?;
    fs::write(destination, archive).map_err(|source| LinkCompatError::Io {
        operation: "failed to write staged archive",
        source,
    })?;

    Ok(LinkCompatReport {
        renamed_occurrences: host_count,
        existing_private_occurrences: private_count,
    })
}

fn validate_archive(archive: &[u8]) -> Result<(), LinkCompatError> {
    if !archive.starts_with(ARCHIVE_MAGIC) {
        return Err(LinkCompatError::InvalidArchive(
            "missing portable archive magic",
        ));
    }

    let mut offset = ARCHIVE_MAGIC.len();
    let mut member_count = 0_usize;
    let mut linker_member_count = 0_usize;
    while offset < archive.len() {
        let header_end = offset
            .checked_add(60)
            .ok_or(LinkCompatError::InvalidArchive("member header overflow"))?;
        let header = archive
            .get(offset..header_end)
            .ok_or(LinkCompatError::InvalidArchive("truncated member header"))?;
        if &header[58..60] != b"`\n" {
            return Err(LinkCompatError::InvalidArchive(
                "invalid member header terminator",
            ));
        }

        let size = parse_decimal(&header[48..58])?;
        let data_end = header_end
            .checked_add(size)
            .ok_or(LinkCompatError::InvalidArchive("member size overflow"))?;
        if data_end > archive.len() {
            return Err(LinkCompatError::InvalidArchive(
                "member extends beyond archive",
            ));
        }
        if is_linker_member_name(&header[..16]) {
            linker_member_count += 1;
            if linker_member_count == 2 {
                validate_msvc_sorted_linker_member(&archive[header_end..data_end])?;
            }
        }

        offset = data_end
            .checked_add(size & 1)
            .ok_or(LinkCompatError::InvalidArchive("member padding overflow"))?;
        if offset > archive.len() {
            return Err(LinkCompatError::InvalidArchive("truncated member padding"));
        }
        member_count += 1;
    }

    if member_count == 0 {
        return Err(LinkCompatError::InvalidArchive("archive has no members"));
    }
    Ok(())
}

fn is_linker_member_name(field: &[u8]) -> bool {
    let end = field
        .iter()
        .rposition(|byte| *byte != b' ')
        .map_or(0, |index| index + 1);
    &field[..end] == b"/"
}

fn validate_msvc_sorted_linker_member(member: &[u8]) -> Result<(), LinkCompatError> {
    let archive_member_count = read_u32_le(member, 0)? as usize;
    if archive_member_count > u16::MAX as usize {
        return Err(LinkCompatError::InvalidArchive(
            "MSVC linker member has too many archive members",
        ));
    }
    let offsets_end =
        4_usize
            .checked_add(archive_member_count.checked_mul(4).ok_or(
                LinkCompatError::InvalidArchive("MSVC linker member offset table overflow"),
            )?)
            .ok_or(LinkCompatError::InvalidArchive(
                "MSVC linker member offset table overflow",
            ))?;
    let symbol_count = read_u32_le(member, offsets_end)? as usize;
    let indices_start = offsets_end
        .checked_add(4)
        .ok_or(LinkCompatError::InvalidArchive(
            "MSVC linker member symbol table overflow",
        ))?;
    let names_start = indices_start
        .checked_add(
            symbol_count
                .checked_mul(2)
                .ok_or(LinkCompatError::InvalidArchive(
                    "MSVC linker member index table overflow",
                ))?,
        )
        .ok_or(LinkCompatError::InvalidArchive(
            "MSVC linker member index table overflow",
        ))?;
    if names_start > member.len() {
        return Err(LinkCompatError::InvalidArchive(
            "truncated MSVC linker member index table",
        ));
    }

    for index in 0..symbol_count {
        let entry = indices_start + index * 2;
        let member_index = u16::from_le_bytes([member[entry], member[entry + 1]]) as usize;
        if member_index == 0 || member_index > archive_member_count {
            return Err(LinkCompatError::InvalidArchive(
                "MSVC linker member index is out of range",
            ));
        }
    }

    let mut name_offset = names_start;
    let mut previous_name: Option<&[u8]> = None;
    for _ in 0..symbol_count {
        let remaining = &member[name_offset..];
        let name_len =
            remaining
                .iter()
                .position(|byte| *byte == 0)
                .ok_or(LinkCompatError::InvalidArchive(
                    "unterminated MSVC linker member symbol",
                ))?;
        let name = &remaining[..name_len];
        if name.is_empty() {
            return Err(LinkCompatError::InvalidArchive(
                "empty MSVC linker member symbol",
            ));
        }
        if previous_name.is_some_and(|previous| previous > name) {
            return Err(LinkCompatError::InvalidArchive(
                "MSVC linker member symbols are not sorted",
            ));
        }
        previous_name = Some(name);
        name_offset =
            name_offset
                .checked_add(name_len + 1)
                .ok_or(LinkCompatError::InvalidArchive(
                    "MSVC linker member symbol table overflow",
                ))?;
    }
    Ok(())
}

fn read_u32_le(bytes: &[u8], offset: usize) -> Result<u32, LinkCompatError> {
    let end = offset
        .checked_add(4)
        .ok_or(LinkCompatError::InvalidArchive(
            "MSVC linker member integer overflow",
        ))?;
    let encoded: [u8; 4] = bytes
        .get(offset..end)
        .ok_or(LinkCompatError::InvalidArchive(
            "truncated MSVC linker member integer",
        ))?
        .try_into()
        .map_err(|_| LinkCompatError::InvalidArchive("invalid MSVC linker member integer"))?;
    Ok(u32::from_le_bytes(encoded))
}

fn parse_decimal(field: &[u8]) -> Result<usize, LinkCompatError> {
    let text = std::str::from_utf8(field)
        .map_err(|_| LinkCompatError::InvalidArchive("member size is not ASCII"))?
        .trim();
    if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(LinkCompatError::InvalidArchive(
            "member size is not decimal",
        ));
    }
    text.parse()
        .map_err(|_| LinkCompatError::InvalidArchive("member size is out of range"))
}

fn count_occurrences(haystack: &[u8], needle: &[u8]) -> usize {
    haystack
        .windows(needle.len())
        .filter(|candidate| *candidate == needle)
        .count()
}

fn replace_equal_length(haystack: &mut [u8], needle: &[u8], replacement: &[u8]) {
    debug_assert_eq!(needle.len(), replacement.len());
    let mut offset = 0_usize;
    while let Some(relative) = haystack[offset..]
        .windows(needle.len())
        .position(|candidate| candidate == needle)
    {
        let start = offset + relative;
        let end = start + needle.len();
        haystack[start..end].copy_from_slice(replacement);
        offset = end;
    }
}
