use std::time::{SystemTime, UNIX_EPOCH};

use op_collab_relay_protocol::{
    LOCATOR_CANONICAL_SIGNING_BYTES, MAX_EXPECTED_DISCOVERY_ID_BYTES, MAX_LOCATOR_KEY_ID_BYTES,
    MAX_PAIRING_LIFETIME_SECS, RELAY_PROTOCOL_VERSION,
};

use crate::{Region, SignerError, SignerResult};

pub const OPLS_VERSION: u8 = 1;
pub const REQUEST_BYTES: usize = 4 + 1 + 1 + 1 + 64 + LOCATOR_CANONICAL_SIGNING_BYTES;
pub const RESPONSE_BYTES: usize = 4 + 1 + 1 + 64;

const REQUEST_MAGIC: &[u8; 4] = b"OPLS";
const RESPONSE_MAGIC: &[u8; 4] = b"OPLR";
const SIGN_OPERATION: u8 = 1;
const STATUS_OK: u8 = 0;
const STATUS_REJECTED: u8 = 1;

const ROUTE_ID_OFFSET: usize = 2;
const GENERATION_OFFSET: usize = 18;
const OWNER_STATIC_OFFSET: usize = 26;
const DISCOVERY_LEN_OFFSET: usize = 58;
const DISCOVERY_OFFSET: usize = 59;
const NOT_BEFORE_OFFSET: usize = 187;
const EXPIRES_OFFSET: usize = 195;
const KEY_ID_LEN_OFFSET: usize = 203;
const KEY_ID_OFFSET: usize = 204;

#[derive(Clone)]
pub struct SignRequest {
    pub canonical: [u8; LOCATOR_CANONICAL_SIGNING_BYTES],
}

pub fn decode_request(bytes: &[u8], region: Region, active_kid: &str) -> SignerResult<SignRequest> {
    if bytes.len() != REQUEST_BYTES
        || &bytes[..4] != REQUEST_MAGIC
        || bytes[4] != OPLS_VERSION
        || bytes[5] != SIGN_OPERATION
    {
        return Err(SignerError::Rejected);
    }
    let kid_len = usize::from(bytes[6]);
    let kid_slot = &bytes[7..7 + MAX_LOCATOR_KEY_ID_BYTES];
    if kid_len == 0
        || kid_len > MAX_LOCATOR_KEY_ID_BYTES
        || kid_slot[kid_len..].iter().any(|byte| *byte != 0)
        || !kid_slot[..kid_len]
            .iter()
            .all(|byte| (0x21..=0x7e).contains(byte))
        || kid_slot[..kid_len] != *active_kid.as_bytes()
    {
        return Err(SignerError::Rejected);
    }
    let canonical: [u8; LOCATOR_CANONICAL_SIGNING_BYTES] = bytes[7 + MAX_LOCATOR_KEY_ID_BYTES..]
        .try_into()
        .map_err(|_| SignerError::Rejected)?;
    validate_locator_domain(&canonical, region, active_kid)?;
    Ok(SignRequest { canonical })
}

pub fn validate_locator_domain(
    canonical: &[u8; LOCATOR_CANONICAL_SIGNING_BYTES],
    region: Region,
    active_kid: &str,
) -> SignerResult<()> {
    if canonical[0] != RELAY_PROTOCOL_VERSION || canonical[1] != region.wire_value() {
        return Err(SignerError::Rejected);
    }
    if canonical[ROUTE_ID_OFFSET..GENERATION_OFFSET]
        .iter()
        .all(|byte| *byte == 0)
        || canonical[GENERATION_OFFSET..OWNER_STATIC_OFFSET]
            .iter()
            .all(|byte| *byte == 0)
        || canonical[OWNER_STATIC_OFFSET..DISCOVERY_LEN_OFFSET]
            .iter()
            .all(|byte| *byte == 0)
    {
        return Err(SignerError::Rejected);
    }
    validate_padded_ascii(
        canonical,
        DISCOVERY_LEN_OFFSET,
        DISCOVERY_OFFSET,
        MAX_EXPECTED_DISCOVERY_ID_BYTES,
        None,
    )?;
    let not_before = read_u64(canonical, NOT_BEFORE_OFFSET);
    let expires = read_u64(canonical, EXPIRES_OFFSET);
    if not_before == 0 || expires <= not_before || expires - not_before > MAX_PAIRING_LIFETIME_SECS
    {
        return Err(SignerError::Rejected);
    }
    validate_padded_ascii(
        canonical,
        KEY_ID_LEN_OFFSET,
        KEY_ID_OFFSET,
        MAX_LOCATOR_KEY_ID_BYTES,
        Some(active_kid.as_bytes()),
    )?;
    Ok(())
}

pub fn canary(
    region: Region,
    active_kid: &str,
) -> SignerResult<[u8; LOCATOR_CANONICAL_SIGNING_BYTES]> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| SignerError::Unavailable("system clock is before Unix epoch".into()))?
        .as_secs()
        .max(1);
    let mut output = [0_u8; LOCATOR_CANONICAL_SIGNING_BYTES];
    output[0] = RELAY_PROTOCOL_VERSION;
    output[1] = region.wire_value();
    output[ROUTE_ID_OFFSET..GENERATION_OFFSET].fill(0x51);
    output[GENERATION_OFFSET..OWNER_STATIC_OFFSET].copy_from_slice(&1_u64.to_be_bytes());
    output[OWNER_STATIC_OFFSET..DISCOVERY_LEN_OFFSET].fill(0x72);
    write_padded_ascii(
        &mut output,
        DISCOVERY_LEN_OFFSET,
        DISCOVERY_OFFSET,
        MAX_EXPECTED_DISCOVERY_ID_BYTES,
        b"opls-readiness-canary-v1",
    );
    output[NOT_BEFORE_OFFSET..EXPIRES_OFFSET].copy_from_slice(&now.to_be_bytes());
    output[EXPIRES_OFFSET..KEY_ID_LEN_OFFSET]
        .copy_from_slice(&now.saturating_add(60).to_be_bytes());
    write_padded_ascii(
        &mut output,
        KEY_ID_LEN_OFFSET,
        KEY_ID_OFFSET,
        MAX_LOCATOR_KEY_ID_BYTES,
        active_kid.as_bytes(),
    );
    validate_locator_domain(&output, region, active_kid)?;
    Ok(output)
}

pub fn success_response(signature: &[u8; 64]) -> [u8; RESPONSE_BYTES] {
    let mut response = [0_u8; RESPONSE_BYTES];
    response[..4].copy_from_slice(RESPONSE_MAGIC);
    response[4] = OPLS_VERSION;
    response[5] = STATUS_OK;
    response[6..].copy_from_slice(signature);
    response
}

pub fn rejected_response() -> [u8; RESPONSE_BYTES] {
    let mut response = [0_u8; RESPONSE_BYTES];
    response[..4].copy_from_slice(RESPONSE_MAGIC);
    response[4] = OPLS_VERSION;
    response[5] = STATUS_REJECTED;
    response
}

fn validate_padded_ascii(
    bytes: &[u8],
    length_offset: usize,
    value_offset: usize,
    maximum: usize,
    expected: Option<&[u8]>,
) -> SignerResult<()> {
    let length = usize::from(bytes[length_offset]);
    let slot = &bytes[value_offset..value_offset + maximum];
    if length == 0
        || length > maximum
        || !slot[..length]
            .iter()
            .all(|byte| (0x21..=0x7e).contains(byte))
        || slot[length..].iter().any(|byte| *byte != 0)
        || expected.is_some_and(|value| value != &slot[..length])
    {
        return Err(SignerError::Rejected);
    }
    Ok(())
}

fn write_padded_ascii(
    bytes: &mut [u8],
    length_offset: usize,
    value_offset: usize,
    maximum: usize,
    value: &[u8],
) {
    debug_assert!(!value.is_empty() && value.len() <= maximum);
    bytes[length_offset] = value.len() as u8;
    bytes[value_offset..value_offset + value.len()].copy_from_slice(value);
}

fn read_u64(bytes: &[u8], offset: usize) -> u64 {
    u64::from_be_bytes(bytes[offset..offset + 8].try_into().expect("fixed offset"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const KID: &str = "locator-global-2026-a";

    fn request(canonical: &[u8; LOCATOR_CANONICAL_SIGNING_BYTES]) -> [u8; REQUEST_BYTES] {
        let mut request = [0_u8; REQUEST_BYTES];
        request[..4].copy_from_slice(REQUEST_MAGIC);
        request[4] = OPLS_VERSION;
        request[5] = SIGN_OPERATION;
        request[6] = KID.len() as u8;
        request[7..7 + KID.len()].copy_from_slice(KID.as_bytes());
        request[7 + MAX_LOCATOR_KEY_ID_BYTES..].copy_from_slice(canonical);
        request
    }

    #[test]
    fn accepts_only_active_locator_domain() {
        let canonical = canary(Region::Global, KID).unwrap();
        assert!(decode_request(&request(&canonical), Region::Global, KID).is_ok());
    }

    #[test]
    fn rejects_wrong_region_and_noncanonical_padding() {
        let mut canonical = canary(Region::Global, KID).unwrap();
        canonical[1] = Region::Cn.wire_value();
        assert!(decode_request(&request(&canonical), Region::Global, KID).is_err());

        let mut canonical = canary(Region::Global, KID).unwrap();
        canonical[KEY_ID_OFFSET + KID.len()] = 1;
        assert!(decode_request(&request(&canonical), Region::Global, KID).is_err());
    }
}
