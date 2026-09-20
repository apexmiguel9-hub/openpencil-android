use std::io::{Cursor, Write};

use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

use crate::{
    decode_html_zip_entry, html_zip_entry_data_range, locate_html_zip_directory,
    parse_html_zip_central_directory, HtmlProjectError, HtmlZipDirectoryLocator,
    HtmlZipRangeManifest, HTML_ZIP_LOCAL_HEADER_FIXED_BYTES, HTML_ZIP_TAIL_BYTES,
};

const CENTRAL_HEADER_FIXED_BYTES: usize = 46;
const INFO_ZIP_UNICODE_PATH_EXTRA_ID: u16 = 0x7075;

fn archive(entries: &[(&str, &[u8], CompressionMethod)]) -> Vec<u8> {
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    for (path, bytes, method) in entries {
        writer
            .start_file(
                *path,
                SimpleFileOptions::default().compression_method(*method),
            )
            .unwrap();
        writer.write_all(bytes).unwrap();
    }
    writer.finish().unwrap().into_inner()
}

fn locator_for(bytes: &[u8]) -> HtmlZipDirectoryLocator {
    let tail_start = bytes.len().saturating_sub(HTML_ZIP_TAIL_BYTES);
    locate_html_zip_directory(bytes.len() as u64, &bytes[tail_start..]).unwrap()
}

fn parse(bytes: &[u8]) -> HtmlZipRangeManifest {
    let locator = locator_for(bytes);
    let start = locator.central_directory_offset as usize;
    let end = start + locator.central_directory_size as usize;
    parse_html_zip_central_directory(&locator, &bytes[start..end]).unwrap()
}

fn decode(bytes: &[u8], manifest: &HtmlZipRangeManifest, index: usize) -> Vec<u8> {
    let entry = &manifest.entries[index];
    let header_start = entry.local_header_offset as usize;
    let header = &bytes[header_start..header_start + HTML_ZIP_LOCAL_HEADER_FIXED_BYTES];
    let range = html_zip_entry_data_range(manifest, entry, header).unwrap();
    let start = range.offset as usize;
    decode_html_zip_entry(entry, header, &bytes[start..start + range.length as usize]).unwrap()
}

fn archive_with_unicode_path_extra(
    raw_name: &[u8],
    unicode_name: &[u8],
    valid_name_crc: bool,
) -> Vec<u8> {
    let placeholder = "x".repeat(raw_name.len().saturating_sub(5)) + ".html";
    assert_eq!(placeholder.len(), raw_name.len());
    let mut bytes = archive(&[(
        placeholder.as_str(),
        b"unicode path",
        CompressionMethod::Stored,
    )]);
    let locator = locator_for(&bytes);
    let central = locator.central_directory_offset as usize;
    let name_len = read_u16(&bytes, central + 28).unwrap() as usize;
    assert_eq!(name_len, raw_name.len());
    assert_eq!(read_u16(&bytes, central + 30), Some(0));
    assert_eq!(read_u16(&bytes, central + 32), Some(0));
    bytes[HTML_ZIP_LOCAL_HEADER_FIXED_BYTES..HTML_ZIP_LOCAL_HEADER_FIXED_BYTES + name_len]
        .copy_from_slice(raw_name);
    bytes[central + CENTRAL_HEADER_FIXED_BYTES..central + CENTRAL_HEADER_FIXED_BYTES + name_len]
        .copy_from_slice(raw_name);

    let mut extra = Vec::with_capacity(9 + unicode_name.len());
    extra.extend_from_slice(&INFO_ZIP_UNICODE_PATH_EXTRA_ID.to_le_bytes());
    extra.extend_from_slice(&(5u16 + unicode_name.len() as u16).to_le_bytes());
    extra.push(1);
    let name_crc = if valid_name_crc {
        crc32(raw_name)
    } else {
        crc32(raw_name) ^ 1
    };
    extra.extend_from_slice(&name_crc.to_le_bytes());
    extra.extend_from_slice(unicode_name);

    let insert_at = central + CENTRAL_HEADER_FIXED_BYTES + name_len;
    bytes.splice(insert_at..insert_at, extra.iter().copied());
    bytes[central + 30..central + 32].copy_from_slice(&(extra.len() as u16).to_le_bytes());
    let shifted_eocd = locator.eocd_offset as usize + extra.len();
    let directory_size = locator.central_directory_size + extra.len() as u64;
    bytes[shifted_eocd + 12..shifted_eocd + 16]
        .copy_from_slice(&(directory_size as u32).to_le_bytes());
    bytes
}

#[test]
fn preserves_utf8_chinese_project_paths() {
    let bytes = archive(&[
        (
            "项目/首页.html",
            b"<link href='styles/site.css'>",
            CompressionMethod::Deflated,
        ),
        (
            "项目/styles/中文.css",
            b"body{color:red}",
            CompressionMethod::Stored,
        ),
    ]);
    let manifest = parse(&bytes);
    assert_eq!(manifest.html_entry().relative_path, "首页.html");
    assert!(manifest
        .entries
        .iter()
        .any(|entry| entry.relative_path == "styles/中文.css"));
    assert_eq!(
        decode(&bytes, &manifest, manifest.html_entry_index),
        b"<link href='styles/site.css'>"
    );
}

#[test]
fn honors_unicode_path_extra_and_validates_its_name_crc() {
    let bytes = archive_with_unicode_path_extra(b"shouye.html", "首页.html".as_bytes(), true);
    let manifest = parse(&bytes);
    assert_eq!(manifest.html_entry().relative_path, "首页.html");

    let bad_crc = archive_with_unicode_path_extra(b"shouye.html", "首页.html".as_bytes(), false);
    let locator = locator_for(&bad_crc);
    let start = locator.central_directory_offset as usize;
    let end = start + locator.central_directory_size as usize;
    assert!(matches!(
        parse_html_zip_central_directory(&locator, &bad_crc[start..end]),
        Err(HtmlProjectError::InvalidZip(detail)) if detail.contains("file-name CRC-32")
    ));

    let bad_utf8 = archive_with_unicode_path_extra(b"shouye.html", b"\xff.html", true);
    let locator = locator_for(&bad_utf8);
    let start = locator.central_directory_offset as usize;
    let end = start + locator.central_directory_size as usize;
    assert!(matches!(
        parse_html_zip_central_directory(&locator, &bad_utf8[start..end]),
        Err(HtmlProjectError::InvalidZip(detail)) if detail.contains("not valid UTF-8")
    ));
}

#[test]
fn rejects_invalid_utf8_names_when_the_utf8_flag_is_set() {
    let mut bytes = archive(&[("index.html", b"ok", CompressionMethod::Stored)]);
    let locator = locator_for(&bytes);
    let central = locator.central_directory_offset as usize;
    bytes[central + 8..central + 10].copy_from_slice(&(1u16 << 11).to_le_bytes());
    bytes[central + CENTRAL_HEADER_FIXED_BYTES] = 0xff;
    let locator = locator_for(&bytes);
    let end = central + locator.central_directory_size as usize;
    assert!(matches!(
        parse_html_zip_central_directory(&locator, &bytes[central..end]),
        Err(HtmlProjectError::InvalidZip(detail)) if detail.contains("not UTF-8")
    ));
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = u32::MAX;
    for byte in bytes {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xedb8_8320 & (0u32.wrapping_sub(crc & 1)));
        }
    }
    !crc
}

fn read_u16(bytes: &[u8], offset: usize) -> Option<u16> {
    Some(u16::from_le_bytes(
        bytes.get(offset..offset + 2)?.try_into().ok()?,
    ))
}
