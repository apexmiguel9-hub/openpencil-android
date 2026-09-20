use crate::HtmlProjectError;

pub(crate) const INFO_ZIP_UNICODE_PATH_EXTRA_ID: u16 = 0x7075;

pub(crate) fn decode_zip_name(
    bytes: &[u8],
    extra: &[u8],
    flags: u16,
) -> Result<String, HtmlProjectError> {
    let unicode_path = parse_unicode_path_extra(bytes, extra)?;
    if flags & (1 << 11) != 0 {
        let utf8_path = std::str::from_utf8(bytes)
            .map_err(|_| invalid_zip("UTF-8 file name flag is set, but the name is not UTF-8"))?;
        if unicode_path
            .as_deref()
            .is_some_and(|path| path != utf8_path)
        {
            return Err(invalid_zip(
                "Unicode Path extra field does not match the UTF-8 file name",
            ));
        }
        return Ok(utf8_path.to_string());
    }
    if let Some(path) = unicode_path {
        return Ok(path);
    }
    Ok(bytes
        .iter()
        .map(|byte| {
            if *byte < 0x80 {
                char::from(*byte)
            } else {
                CP437_HIGH[usize::from(*byte - 0x80)]
            }
        })
        .collect())
}

fn parse_unicode_path_extra(
    raw_name: &[u8],
    extra: &[u8],
) -> Result<Option<String>, HtmlProjectError> {
    let mut unicode_path = None;
    let mut offset = 0usize;
    while offset < extra.len() {
        let header_end = offset
            .checked_add(4)
            .ok_or_else(|| invalid_zip("extra-field bounds overflow"))?;
        let header = extra
            .get(offset..header_end)
            .ok_or_else(|| invalid_zip("extra-field header is truncated"))?;
        let field_id =
            read_u16(header, 0).ok_or_else(|| invalid_zip("extra-field header is truncated"))?;
        let field_len = read_u16(header, 2)
            .ok_or_else(|| invalid_zip("extra-field header is truncated"))?
            as usize;
        let field_end = header_end
            .checked_add(field_len)
            .ok_or_else(|| invalid_zip("extra-field bounds overflow"))?;
        let field = extra
            .get(header_end..field_end)
            .ok_or_else(|| invalid_zip("extra-field payload is truncated"))?;
        offset = field_end;

        if field_id != INFO_ZIP_UNICODE_PATH_EXTRA_ID {
            continue;
        }
        if unicode_path.is_some() {
            return Err(invalid_zip("duplicate Unicode Path extra field"));
        }
        if field.len() < 5 {
            return Err(invalid_zip("Unicode Path extra field is truncated"));
        }
        if field[0] != 1 {
            return Err(invalid_zip(
                "Unicode Path extra field version is not supported",
            ));
        }
        let expected_crc = read_u32(field, 1)
            .ok_or_else(|| invalid_zip("Unicode Path extra field is truncated"))?;
        if expected_crc != crc32(raw_name) {
            return Err(invalid_zip(
                "Unicode Path extra field file-name CRC-32 does not match",
            ));
        }
        let path = std::str::from_utf8(&field[5..])
            .map_err(|_| invalid_zip("Unicode Path extra field is not valid UTF-8"))?;
        unicode_path = Some(path.to_string());
    }
    Ok(unicode_path)
}

const CP437_HIGH: [char; 128] = [
    'Ç', 'ü', 'é', 'â', 'ä', 'à', 'å', 'ç', 'ê', 'ë', 'è', 'ï', 'î', 'ì', 'Ä', 'Å', 'É', 'æ', 'Æ',
    'ô', 'ö', 'ò', 'û', 'ù', 'ÿ', 'Ö', 'Ü', '¢', '£', '¥', '₧', 'ƒ', 'á', 'í', 'ó', 'ú', 'ñ', 'Ñ',
    'ª', 'º', '¿', '⌐', '¬', '½', '¼', '¡', '«', '»', '░', '▒', '▓', '│', '┤', '╡', '╢', '╖', '╕',
    '╣', '║', '╗', '╝', '╜', '╛', '┐', '└', '┴', '┬', '├', '─', '┼', '╞', '╟', '╚', '╔', '╩', '╦',
    '╠', '═', '╬', '╧', '╨', '╤', '╥', '╙', '╘', '╒', '╓', '╫', '╪', '┘', '┌', '█', '▄', '▌', '▐',
    '▀', 'α', 'ß', 'Γ', 'π', 'Σ', 'σ', 'µ', 'τ', 'Φ', 'Θ', 'Ω', 'δ', '∞', 'φ', 'ε', '∩', '≡', '±',
    '≥', '≤', '⌠', '⌡', '÷', '≈', '°', '∙', '·', '√', 'ⁿ', '²', '■', ' ',
];

pub(crate) fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = u32::MAX;
    for byte in bytes {
        crc = (crc >> 8) ^ CRC32_TABLE[((crc ^ u32::from(*byte)) & 0xff) as usize];
    }
    !crc
}

const CRC32_TABLE: [u32; 256] = crc32_table();

const fn crc32_table() -> [u32; 256] {
    let mut table = [0u32; 256];
    let mut index = 0usize;
    while index < table.len() {
        let mut value = index as u32;
        let mut bit = 0;
        while bit < 8 {
            value = (value >> 1) ^ (0xedb8_8320 & (0u32.wrapping_sub(value & 1)));
            bit += 1;
        }
        table[index] = value;
        index += 1;
    }
    table
}

fn invalid_zip(detail: &str) -> HtmlProjectError {
    HtmlProjectError::InvalidZip(detail.to_string())
}

fn read_u16(bytes: &[u8], offset: usize) -> Option<u16> {
    Some(u16::from_le_bytes(
        bytes.get(offset..offset + 2)?.try_into().ok()?,
    ))
}

fn read_u32(bytes: &[u8], offset: usize) -> Option<u32> {
    Some(u32::from_le_bytes(
        bytes.get(offset..offset + 4)?.try_into().ok()?,
    ))
}
