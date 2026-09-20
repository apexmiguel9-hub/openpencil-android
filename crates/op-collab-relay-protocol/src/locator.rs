use std::fmt;
use std::num::NonZeroU64;

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};

use crate::{OwnerNoiseStatic, RelayProtocolError, RouteCapability, RouteId, RouteMapKey};

pub const RELAY_PROTOCOL_VERSION: u8 = 1;
pub const LOCATOR_PREFIX: &str = "opcl1_";
pub const MAX_INVITE_CHARS: usize = 512;
pub const MAX_EXPECTED_DISCOVERY_ID_BYTES: usize = 128;
pub const MAX_LOCATOR_KEY_ID_BYTES: usize = 64;
pub const MAX_PAIRING_LIFETIME_SECS: u64 = 60 * 60;
pub const LOCATOR_CANONICAL_SIGNING_BYTES: usize = 268;
pub const RELAY_LOCATOR_BINARY_BYTES: usize = 336;

const ROUTE_ID_OFFSET: usize = 2;
const GENERATION_OFFSET: usize = 18;
const OWNER_STATIC_OFFSET: usize = 26;
const DISCOVERY_LEN_OFFSET: usize = 58;
const DISCOVERY_OFFSET: usize = 59;
const NOT_BEFORE_OFFSET: usize = 187;
const EXPIRES_OFFSET: usize = 195;
const KEY_ID_LEN_OFFSET: usize = 203;
const KEY_ID_OFFSET: usize = 204;
const SIGNATURE_OFFSET: usize = LOCATOR_CANONICAL_SIGNING_BYTES;
const CHECKSUM_OFFSET: usize = 332;
const CHECKSUM_CONTEXT: &str = "openpencil/op-collab-relay-protocol/locator-corruption-checksum/v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum RelayRegion {
    Cn = 1,
    Global = 2,
}

impl TryFrom<u8> for RelayRegion {
    type Error = RelayProtocolError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::Cn),
            2 => Ok(Self::Global),
            other => Err(RelayProtocolError::InvalidRegion(other)),
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct ExpectedDiscoveryId(String);

impl ExpectedDiscoveryId {
    pub fn new(value: impl Into<String>) -> Result<Self, RelayProtocolError> {
        let value = value.into();
        validate_ascii(
            "expected_discovery_id",
            &value,
            MAX_EXPECTED_DISCOVERY_ID_BYTES,
        )?;
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for ExpectedDiscoveryId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ExpectedDiscoveryId([REDACTED])")
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct LocatorKeyId(String);

impl LocatorKeyId {
    pub fn new(value: impl Into<String>) -> Result<Self, RelayProtocolError> {
        let value = value.into();
        validate_ascii("key_id", &value, MAX_LOCATOR_KEY_ID_BYTES)?;
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for LocatorKeyId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("LocatorKeyId([REDACTED])")
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct LocatorSignature([u8; 64]);

impl LocatorSignature {
    pub fn new(bytes: [u8; 64]) -> Result<Self, RelayProtocolError> {
        if bytes.iter().all(|byte| *byte == 0) {
            return Err(RelayProtocolError::ZeroSignature);
        }
        Ok(Self(bytes))
    }

    pub const fn as_bytes(&self) -> &[u8; 64] {
        &self.0
    }
}

impl fmt::Debug for LocatorSignature {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("LocatorSignature([REDACTED])")
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct UnsignedRelayLocatorV1 {
    home_region: RelayRegion,
    route_id: RouteId,
    generation: NonZeroU64,
    owner_noise_static: OwnerNoiseStatic,
    expected_discovery_id: ExpectedDiscoveryId,
    not_before_unix: u64,
    expires_at_unix: u64,
    key_id: LocatorKeyId,
}

impl UnsignedRelayLocatorV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        home_region: RelayRegion,
        route_id: RouteId,
        generation: NonZeroU64,
        owner_noise_static: OwnerNoiseStatic,
        expected_discovery_id: ExpectedDiscoveryId,
        not_before_unix: u64,
        expires_at_unix: u64,
        key_id: LocatorKeyId,
    ) -> Result<Self, RelayProtocolError> {
        validate_declared_window(not_before_unix, expires_at_unix)?;
        Ok(Self {
            home_region,
            route_id,
            generation,
            owner_noise_static,
            expected_discovery_id,
            not_before_unix,
            expires_at_unix,
            key_id,
        })
    }

    pub const fn home_region(&self) -> RelayRegion {
        self.home_region
    }

    pub const fn route_id(&self) -> &RouteId {
        &self.route_id
    }

    pub const fn generation(&self) -> NonZeroU64 {
        self.generation
    }

    pub const fn owner_noise_static(&self) -> &OwnerNoiseStatic {
        &self.owner_noise_static
    }

    pub fn expected_discovery_id(&self) -> &ExpectedDiscoveryId {
        &self.expected_discovery_id
    }

    pub const fn not_before_unix(&self) -> u64 {
        self.not_before_unix
    }

    pub const fn expires_at_unix(&self) -> u64 {
        self.expires_at_unix
    }

    pub fn key_id(&self) -> &LocatorKeyId {
        &self.key_id
    }

    pub fn canonical_signing_bytes(&self) -> [u8; LOCATOR_CANONICAL_SIGNING_BYTES] {
        let mut bytes = [0_u8; LOCATOR_CANONICAL_SIGNING_BYTES];
        encode_claims(self, &mut bytes);
        bytes
    }

    pub fn attach_signature(self, signature: LocatorSignature) -> RelayLocatorV1 {
        RelayLocatorV1 {
            claims: self,
            signature,
        }
    }

    pub fn validate_pairing_window(&self, now_unix: u64) -> Result<(), RelayProtocolError> {
        validate_pairing_window(self.not_before_unix, self.expires_at_unix, now_unix)
    }
}

impl fmt::Debug for UnsignedRelayLocatorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("UnsignedRelayLocatorV1")
            .field("home_region", &self.home_region)
            .field("route_id", &self.route_id)
            .field("generation", &self.generation)
            .field("owner_noise_static", &self.owner_noise_static)
            .field("expected_discovery_id", &self.expected_discovery_id)
            .field("not_before_unix", &self.not_before_unix)
            .field("expires_at_unix", &self.expires_at_unix)
            .field("key_id", &self.key_id)
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct RelayLocatorV1 {
    claims: UnsignedRelayLocatorV1,
    signature: LocatorSignature,
}

impl RelayLocatorV1 {
    pub fn claims(&self) -> &UnsignedRelayLocatorV1 {
        &self.claims
    }

    pub fn signature(&self) -> &LocatorSignature {
        &self.signature
    }

    pub fn canonical_signing_bytes(&self) -> [u8; LOCATOR_CANONICAL_SIGNING_BYTES] {
        self.claims.canonical_signing_bytes()
    }

    pub fn encode(&self) -> String {
        let encoded = URL_SAFE_NO_PAD.encode(self.to_binary());
        debug_assert!(LOCATOR_PREFIX.len() + encoded.len() <= MAX_INVITE_CHARS);
        format!("{LOCATOR_PREFIX}{encoded}")
    }

    pub fn decode(encoded: &str) -> Result<Self, RelayProtocolError> {
        if encoded.len() > MAX_INVITE_CHARS {
            return Err(RelayProtocolError::LocatorTooLong {
                actual: encoded.len(),
                maximum: MAX_INVITE_CHARS,
            });
        }
        let body = encoded
            .strip_prefix(LOCATOR_PREFIX)
            .ok_or(RelayProtocolError::InvalidLocatorPrefix)?;
        if body.is_empty()
            || body.contains('=')
            || !body
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
        {
            return Err(RelayProtocolError::InvalidLocatorEncoding);
        }
        let raw = URL_SAFE_NO_PAD
            .decode(body)
            .map_err(|_| RelayProtocolError::InvalidLocatorEncoding)?;
        if URL_SAFE_NO_PAD.encode(&raw) != body {
            return Err(RelayProtocolError::InvalidLocatorEncoding);
        }
        Self::decode_binary(&raw)
    }

    pub fn verify(
        &self,
        verifier: &impl RelayLocatorVerifier,
        now_unix: u64,
    ) -> Result<VerifiedRelayLocator, RelayProtocolError> {
        self.claims.validate_pairing_window(now_unix)?;
        let canonical = self.canonical_signing_bytes();
        if !verifier.verify(self.claims.key_id(), &canonical, self.signature.as_bytes()) {
            return Err(RelayProtocolError::SignatureVerificationFailed);
        }
        Ok(VerifiedRelayLocator(self.clone()))
    }

    pub(crate) fn to_binary(&self) -> [u8; RELAY_LOCATOR_BINARY_BYTES] {
        let mut raw = [0_u8; RELAY_LOCATOR_BINARY_BYTES];
        encode_claims(&self.claims, &mut raw[..LOCATOR_CANONICAL_SIGNING_BYTES]);
        raw[SIGNATURE_OFFSET..CHECKSUM_OFFSET].copy_from_slice(self.signature.as_bytes());
        let checksum = corruption_checksum(&raw[..CHECKSUM_OFFSET]);
        raw[CHECKSUM_OFFSET..].copy_from_slice(&checksum);
        raw
    }

    pub(crate) fn decode_binary(raw: &[u8]) -> Result<Self, RelayProtocolError> {
        require_exact("relay locator", raw.len(), RELAY_LOCATOR_BINARY_BYTES)?;
        if raw[0] != RELAY_PROTOCOL_VERSION {
            return Err(RelayProtocolError::UnsupportedVersion {
                actual: raw[0],
                expected: RELAY_PROTOCOL_VERSION,
            });
        }
        let home_region = RelayRegion::try_from(raw[1])?;
        let route_id = RouteId::new(read_array(raw, ROUTE_ID_OFFSET)?)?;
        let generation = NonZeroU64::new(read_u64(raw, GENERATION_OFFSET)?)
            .ok_or(RelayProtocolError::ZeroGeneration)?;
        let owner_noise_static = OwnerNoiseStatic::new(read_array(raw, OWNER_STATIC_OFFSET)?)?;
        let discovery_len = usize::from(raw[DISCOVERY_LEN_OFFSET]);
        let expected_discovery_id = decode_padded_ascii(
            "expected_discovery_id",
            raw,
            DISCOVERY_OFFSET,
            MAX_EXPECTED_DISCOVERY_ID_BYTES,
            discovery_len,
        )?;
        let not_before_unix = read_u64(raw, NOT_BEFORE_OFFSET)?;
        let expires_at_unix = read_u64(raw, EXPIRES_OFFSET)?;
        let key_id_len = usize::from(raw[KEY_ID_LEN_OFFSET]);
        let key_id = decode_padded_ascii(
            "key_id",
            raw,
            KEY_ID_OFFSET,
            MAX_LOCATOR_KEY_ID_BYTES,
            key_id_len,
        )?;
        let signature = LocatorSignature::new(read_array(raw, SIGNATURE_OFFSET)?)?;
        let claims = UnsignedRelayLocatorV1::new(
            home_region,
            route_id,
            generation,
            owner_noise_static,
            ExpectedDiscoveryId::new(expected_discovery_id)?,
            not_before_unix,
            expires_at_unix,
            LocatorKeyId::new(key_id)?,
        )?;
        let expected_checksum = corruption_checksum(&raw[..CHECKSUM_OFFSET]);
        if raw[CHECKSUM_OFFSET..] != expected_checksum {
            return Err(RelayProtocolError::ChecksumMismatch);
        }
        Ok(Self { claims, signature })
    }
}

impl fmt::Debug for RelayLocatorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RelayLocatorV1")
            .field("claims", &self.claims)
            .field("signature", &self.signature)
            .finish()
    }
}

pub trait RelayLocatorVerifier {
    fn verify(
        &self,
        key_id: &LocatorKeyId,
        canonical_signing_bytes: &[u8],
        signature: &[u8; 64],
    ) -> bool;
}

#[derive(Clone, PartialEq, Eq)]
pub struct VerifiedRelayLocator(RelayLocatorV1);

impl VerifiedRelayLocator {
    pub fn locator(&self) -> &RelayLocatorV1 {
        &self.0
    }

    pub fn claims(&self) -> &UnsignedRelayLocatorV1 {
        self.0.claims()
    }
}

impl fmt::Debug for VerifiedRelayLocator {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("VerifiedRelayLocator")
            .field(&self.0)
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct VerifiedRelayRoute {
    locator: VerifiedRelayLocator,
    capability: RouteCapability,
}

impl VerifiedRelayRoute {
    pub fn new(locator: VerifiedRelayLocator, capability: RouteCapability) -> Self {
        Self {
            locator,
            capability,
        }
    }

    pub fn locator(&self) -> &VerifiedRelayLocator {
        &self.locator
    }

    pub fn route_map_key(&self) -> RouteMapKey {
        RouteMapKey::derive(
            self.locator.claims().route_id(),
            self.locator.claims().generation(),
            &self.capability,
        )
    }

    pub(crate) fn capability(&self) -> &RouteCapability {
        &self.capability
    }
}

impl fmt::Debug for VerifiedRelayRoute {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VerifiedRelayRoute")
            .field("locator", &self.locator)
            .field("capability", &self.capability)
            .finish()
    }
}

fn validate_ascii(
    field: &'static str,
    value: &str,
    maximum: usize,
) -> Result<(), RelayProtocolError> {
    if value.len() > maximum {
        return Err(RelayProtocolError::AsciiFieldTooLong {
            field,
            actual: value.len(),
            maximum,
        });
    }
    if value.is_empty() || !value.bytes().all(|byte| (0x21..=0x7e).contains(&byte)) {
        return Err(RelayProtocolError::InvalidAsciiField { field });
    }
    Ok(())
}

fn validate_declared_window(
    not_before_unix: u64,
    expires_at_unix: u64,
) -> Result<(), RelayProtocolError> {
    if not_before_unix == 0 {
        return Err(RelayProtocolError::ZeroNotBefore);
    }
    if expires_at_unix <= not_before_unix {
        return Err(RelayProtocolError::InvalidExpiry);
    }
    Ok(())
}

fn validate_pairing_window(
    not_before_unix: u64,
    expires_at_unix: u64,
    now_unix: u64,
) -> Result<(), RelayProtocolError> {
    validate_declared_window(not_before_unix, expires_at_unix)?;
    if now_unix < not_before_unix {
        return Err(RelayProtocolError::NotYetValid);
    }
    if expires_at_unix <= now_unix {
        return Err(RelayProtocolError::Expired);
    }
    if expires_at_unix - now_unix > MAX_PAIRING_LIFETIME_SECS {
        return Err(RelayProtocolError::ExpiryTooFarFuture);
    }
    if expires_at_unix - not_before_unix > MAX_PAIRING_LIFETIME_SECS {
        return Err(RelayProtocolError::ValidityWindowTooLong);
    }
    Ok(())
}

fn encode_claims(claims: &UnsignedRelayLocatorV1, output: &mut [u8]) {
    debug_assert_eq!(output.len(), LOCATOR_CANONICAL_SIGNING_BYTES);
    output[0] = RELAY_PROTOCOL_VERSION;
    output[1] = claims.home_region as u8;
    output[ROUTE_ID_OFFSET..GENERATION_OFFSET].copy_from_slice(claims.route_id.as_bytes());
    output[GENERATION_OFFSET..OWNER_STATIC_OFFSET]
        .copy_from_slice(&claims.generation.get().to_be_bytes());
    output[OWNER_STATIC_OFFSET..DISCOVERY_LEN_OFFSET]
        .copy_from_slice(claims.owner_noise_static.as_bytes());
    encode_padded_ascii(
        claims.expected_discovery_id.as_str(),
        output,
        DISCOVERY_LEN_OFFSET,
        DISCOVERY_OFFSET,
        MAX_EXPECTED_DISCOVERY_ID_BYTES,
    );
    output[NOT_BEFORE_OFFSET..EXPIRES_OFFSET]
        .copy_from_slice(&claims.not_before_unix.to_be_bytes());
    output[EXPIRES_OFFSET..KEY_ID_LEN_OFFSET]
        .copy_from_slice(&claims.expires_at_unix.to_be_bytes());
    encode_padded_ascii(
        claims.key_id.as_str(),
        output,
        KEY_ID_LEN_OFFSET,
        KEY_ID_OFFSET,
        MAX_LOCATOR_KEY_ID_BYTES,
    );
}

fn encode_padded_ascii(
    value: &str,
    output: &mut [u8],
    length_offset: usize,
    value_offset: usize,
    maximum: usize,
) {
    output[length_offset] = value.len() as u8;
    output[value_offset..value_offset + value.len()].copy_from_slice(value.as_bytes());
    output[value_offset + value.len()..value_offset + maximum].fill(0);
}

fn decode_padded_ascii(
    field: &'static str,
    raw: &[u8],
    offset: usize,
    maximum: usize,
    length: usize,
) -> Result<String, RelayProtocolError> {
    if length > maximum {
        return Err(RelayProtocolError::AsciiFieldTooLong {
            field,
            actual: length,
            maximum,
        });
    }
    let padded = &raw[offset..offset + maximum];
    if padded[length..].iter().any(|byte| *byte != 0) {
        return Err(RelayProtocolError::NonZeroReserved);
    }
    let value = std::str::from_utf8(&padded[..length])
        .map_err(|_| RelayProtocolError::InvalidAsciiField { field })?;
    validate_ascii(field, value, maximum)?;
    Ok(value.to_owned())
}

fn corruption_checksum(bytes: &[u8]) -> [u8; 4] {
    let mut hasher = blake3::Hasher::new_derive_key(CHECKSUM_CONTEXT);
    hasher.update(bytes);
    let digest = hasher.finalize();
    let mut checksum = [0_u8; 4];
    checksum.copy_from_slice(&digest.as_bytes()[..4]);
    checksum
}

fn read_u64(raw: &[u8], offset: usize) -> Result<u64, RelayProtocolError> {
    Ok(u64::from_be_bytes(read_array(raw, offset)?))
}

fn read_array<const N: usize>(raw: &[u8], offset: usize) -> Result<[u8; N], RelayProtocolError> {
    raw.get(offset..offset + N)
        .and_then(|slice| slice.try_into().ok())
        .ok_or(RelayProtocolError::Truncated {
            context: "relay locator",
            expected: offset + N,
            actual: raw.len(),
        })
}

pub(crate) fn require_exact(
    context: &'static str,
    actual: usize,
    expected: usize,
) -> Result<(), RelayProtocolError> {
    if actual < expected {
        return Err(RelayProtocolError::Truncated {
            context,
            expected,
            actual,
        });
    }
    if actual > expected {
        return Err(RelayProtocolError::TrailingBytes {
            context,
            expected,
            actual,
        });
    }
    Ok(())
}
