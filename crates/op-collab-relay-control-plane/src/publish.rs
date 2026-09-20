use std::{
    fmt,
    num::{NonZeroU32, NonZeroU64},
    time::{SystemTime, UNIX_EPOCH},
};

use op_collab_relay_protocol::{
    ExpectedDiscoveryId, OwnerNoiseStatic, RelayInviteV1, RelayLocatorVerifier, RelayRegion,
    RouteCapability, RouteId, MAX_EXPECTED_DISCOVERY_ID_BYTES, MAX_PAIRING_LIFETIME_SECS,
    RELAY_PROTOCOL_VERSION,
};
use subtle::ConstantTimeEq as _;
use zeroize::{Zeroize, Zeroizing};

use crate::{RelayLocatorIssueError, SignedLocatorResponse};

pub const OWNER_PUBLISH_REQUEST_BYTES: usize = 191;
pub const MAX_ISSUER_CLOCK_SKEW_SECS: u64 = 5 * 60;

const ROUTE_ID_OFFSET: usize = 2;
const GENERATION_OFFSET: usize = 18;
const OWNER_STATIC_OFFSET: usize = 26;
const DISCOVERY_LEN_OFFSET: usize = 58;
const DISCOVERY_OFFSET: usize = 59;
const LIFETIME_OFFSET: usize = 187;

// The discovery-id field is exactly as wide as the longest id the protocol
// admits, which is what makes the `DISCOVERY_OFFSET + discovery_len` slices
// below infallible for any length that passes the bound check. Growing the
// protocol constant without widening the field would turn a single byte of a
// request into an out-of-range slice, so fail the build instead.
const _: () = assert!(LIFETIME_OFFSET - DISCOVERY_OFFSET == MAX_EXPECTED_DISCOVERY_ID_BYTES);
const _: () = assert!(LIFETIME_OFFSET + 4 == OWNER_PUBLISH_REQUEST_BYTES);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RelayPublishLifetime(NonZeroU32);

impl RelayPublishLifetime {
    pub fn new(seconds: u64) -> Result<Self, RelayLocatorIssueError> {
        let seconds = NonZeroU32::new(u32::try_from(seconds).map_err(|_| {
            RelayLocatorIssueError::LifetimeTooLong {
                actual: seconds,
                maximum: MAX_PAIRING_LIFETIME_SECS,
            }
        })?)
        .ok_or(RelayLocatorIssueError::ZeroLifetime)?;
        if u64::from(seconds.get()) > MAX_PAIRING_LIFETIME_SECS {
            return Err(RelayLocatorIssueError::LifetimeTooLong {
                actual: u64::from(seconds.get()),
                maximum: MAX_PAIRING_LIFETIME_SECS,
            });
        }
        Ok(Self(seconds))
    }

    pub const fn seconds(self) -> u64 {
        self.0.get() as u64
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct OwnerPublishRequest {
    home_region: RelayRegion,
    route_id: RouteId,
    generation: NonZeroU64,
    owner_noise_static: OwnerNoiseStatic,
    expected_discovery_id: ExpectedDiscoveryId,
    desired_lifetime: RelayPublishLifetime,
}

impl OwnerPublishRequest {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        home_region: RelayRegion,
        route_id: RouteId,
        generation: NonZeroU64,
        owner_noise_static: OwnerNoiseStatic,
        expected_discovery_id: ExpectedDiscoveryId,
        desired_lifetime: RelayPublishLifetime,
    ) -> Self {
        Self {
            home_region,
            route_id,
            generation,
            owner_noise_static,
            expected_discovery_id,
            desired_lifetime,
        }
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

    pub const fn desired_lifetime(&self) -> RelayPublishLifetime {
        self.desired_lifetime
    }

    /// Encodes the bounded HTTPS request body.
    ///
    /// The bearer route capability is intentionally absent: it remains only
    /// on the owner device and is never disclosed to the locator issuer.
    pub fn encode_binary(&self) -> [u8; OWNER_PUBLISH_REQUEST_BYTES] {
        let mut output = [0_u8; OWNER_PUBLISH_REQUEST_BYTES];
        output[0] = RELAY_PROTOCOL_VERSION;
        output[1] = self.home_region as u8;
        output[ROUTE_ID_OFFSET..GENERATION_OFFSET].copy_from_slice(self.route_id.as_bytes());
        output[GENERATION_OFFSET..OWNER_STATIC_OFFSET]
            .copy_from_slice(&self.generation.get().to_be_bytes());
        output[OWNER_STATIC_OFFSET..DISCOVERY_LEN_OFFSET]
            .copy_from_slice(self.owner_noise_static.as_bytes());
        let discovery = self.expected_discovery_id.as_str().as_bytes();
        output[DISCOVERY_LEN_OFFSET] =
            u8::try_from(discovery.len()).expect("bounded discovery id fits u8");
        output[DISCOVERY_OFFSET..DISCOVERY_OFFSET + discovery.len()].copy_from_slice(discovery);
        output[LIFETIME_OFFSET..]
            .copy_from_slice(&(self.desired_lifetime.seconds() as u32).to_be_bytes());
        output
    }

    pub fn decode_binary(input: &[u8]) -> Result<Self, RelayLocatorIssueError> {
        if input.len() != OWNER_PUBLISH_REQUEST_BYTES {
            return Err(RelayLocatorIssueError::InvalidRequestLength {
                actual: input.len(),
                expected: OWNER_PUBLISH_REQUEST_BYTES,
            });
        }
        if input[0] != RELAY_PROTOCOL_VERSION {
            return Err(RelayLocatorIssueError::UnsupportedRequestVersion {
                actual: input[0],
                expected: RELAY_PROTOCOL_VERSION,
            });
        }
        let home_region = RelayRegion::try_from(input[1])?;
        let route_id = RouteId::new(read_array(input, ROUTE_ID_OFFSET)?)?;
        let generation = NonZeroU64::new(u64::from_be_bytes(read_array(input, GENERATION_OFFSET)?))
            .ok_or(op_collab_relay_protocol::RelayProtocolError::ZeroGeneration)?;
        let owner_noise_static = OwnerNoiseStatic::new(read_array(input, OWNER_STATIC_OFFSET)?)?;
        let discovery_len = usize::from(input[DISCOVERY_LEN_OFFSET]);
        if discovery_len > MAX_EXPECTED_DISCOVERY_ID_BYTES {
            return Err(
                op_collab_relay_protocol::RelayProtocolError::AsciiFieldTooLong {
                    field: "expected_discovery_id",
                    actual: discovery_len,
                    maximum: MAX_EXPECTED_DISCOVERY_ID_BYTES,
                }
                .into(),
            );
        }
        let discovery_padding = &input[DISCOVERY_OFFSET + discovery_len..LIFETIME_OFFSET];
        if discovery_padding.iter().any(|byte| *byte != 0) {
            return Err(RelayLocatorIssueError::NonZeroRequestPadding);
        }
        let discovery =
            std::str::from_utf8(&input[DISCOVERY_OFFSET..DISCOVERY_OFFSET + discovery_len])
                .map_err(|_| RelayLocatorIssueError::InvalidRequestUtf8)?;
        let expected_discovery_id = ExpectedDiscoveryId::new(discovery)?;
        let desired_lifetime = RelayPublishLifetime::new(u64::from(u32::from_be_bytes(
            read_array(input, LIFETIME_OFFSET)?,
        )))?;
        Ok(Self::new(
            home_region,
            route_id,
            generation,
            owner_noise_static,
            expected_discovery_id,
            desired_lifetime,
        ))
    }
}

impl fmt::Debug for OwnerPublishRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OwnerPublishRequest")
            .field("home_region", &self.home_region)
            .field("route_id", &"[REDACTED]")
            .field("generation", &"[REDACTED]")
            .field("owner_noise_static", &"[REDACTED]")
            .field("expected_discovery_id", &"[REDACTED]")
            .field("desired_lifetime", &self.desired_lifetime)
            .finish()
    }
}

/// Owner-local pending publish state.
///
/// The capability is generated locally, never encoded in
/// [`OwnerPublishRequest`], and wiped by its protocol type when this draft is
/// dropped.
pub struct OwnerPublishDraft {
    request: OwnerPublishRequest,
    capability: RouteCapability,
    created_at_unix: u64,
}

impl OwnerPublishDraft {
    pub fn generate(
        home_region: RelayRegion,
        owner_noise_static: OwnerNoiseStatic,
        expected_discovery_id: ExpectedDiscoveryId,
        desired_lifetime: RelayPublishLifetime,
    ) -> Result<Self, RelayLocatorIssueError> {
        Self::generate_at(
            home_region,
            owner_noise_static,
            expected_discovery_id,
            desired_lifetime,
            unix_now()?,
        )
    }

    pub fn generate_at(
        home_region: RelayRegion,
        owner_noise_static: OwnerNoiseStatic,
        expected_discovery_id: ExpectedDiscoveryId,
        desired_lifetime: RelayPublishLifetime,
        created_at_unix: u64,
    ) -> Result<Self, RelayLocatorIssueError> {
        if created_at_unix == 0 {
            return Err(RelayLocatorIssueError::ZeroCurrentTime);
        }
        let route_id = RouteId::generate()?;
        let capability = RouteCapability::generate()?;
        let generation = random_generation()?;
        Ok(Self {
            request: OwnerPublishRequest::new(
                home_region,
                route_id,
                generation,
                owner_noise_static,
                expected_discovery_id,
                desired_lifetime,
            ),
            capability,
            created_at_unix,
        })
    }

    pub fn request(&self) -> &OwnerPublishRequest {
        &self.request
    }

    pub fn complete(
        self,
        response: SignedLocatorResponse,
        verifier: &impl RelayLocatorVerifier,
        now_unix: u64,
    ) -> Result<SignedInviteResponse, RelayLocatorIssueError> {
        let verified = response.locator.verify(verifier, now_unix)?;
        let claims = verified.claims();
        require_match(
            claims.home_region() == self.request.home_region,
            "home_region",
        )?;
        require_match(claims.route_id() == &self.request.route_id, "route_id")?;
        require_match(claims.generation() == self.request.generation, "generation")?;
        require_match(
            bool::from(
                claims
                    .owner_noise_static()
                    .as_bytes()
                    .ct_eq(self.request.owner_noise_static.as_bytes()),
            ),
            "owner_noise_static",
        )?;
        require_match(
            claims.expected_discovery_id() == &self.request.expected_discovery_id,
            "expected_discovery_id",
        )?;
        require_match(
            claims.not_before_unix()
                >= self
                    .created_at_unix
                    .saturating_sub(MAX_ISSUER_CLOCK_SKEW_SECS),
            "not_before_unix",
        )?;
        require_match(
            claims
                .expires_at_unix()
                .saturating_sub(claims.not_before_unix())
                <= self.request.desired_lifetime.seconds(),
            "expires_at_unix",
        )?;
        let expires_at_unix = claims.expires_at_unix();
        let home_region = claims.home_region();
        let invite = RelayInviteV1::from_signed_locator(response.locator, self.capability);
        Ok(SignedInviteResponse {
            invite,
            home_region,
            expires_at_unix,
        })
    }
}

impl fmt::Debug for OwnerPublishDraft {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("OwnerPublishDraft([REDACTED])")
    }
}

pub struct SignedInviteResponse {
    invite: RelayInviteV1,
    home_region: RelayRegion,
    expires_at_unix: u64,
}

impl SignedInviteResponse {
    pub fn invite(&self) -> &RelayInviteV1 {
        &self.invite
    }

    pub fn invite_code(&self) -> SecretInviteCode {
        SecretInviteCode(Zeroizing::new(self.invite.to_fragment()))
    }

    pub const fn home_region(&self) -> RelayRegion {
        self.home_region
    }

    pub const fn expires_at_unix(&self) -> u64 {
        self.expires_at_unix
    }
}

impl fmt::Debug for SignedInviteResponse {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SignedInviteResponse")
            .field("invite", &"[REDACTED]")
            .field("home_region", &self.home_region)
            .field("expires_at_unix", &self.expires_at_unix)
            .finish()
    }
}

pub struct SecretInviteCode(Zeroizing<String>);

impl SecretInviteCode {
    pub fn expose_secret(&self) -> &str {
        self.0.as_str()
    }
}

impl fmt::Debug for SecretInviteCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SecretInviteCode([REDACTED])")
    }
}

fn require_match(matches: bool, field: &'static str) -> Result<(), RelayLocatorIssueError> {
    if matches {
        Ok(())
    } else {
        Err(RelayLocatorIssueError::ResponseBindingMismatch { field })
    }
}

fn random_generation() -> Result<NonZeroU64, RelayLocatorIssueError> {
    const MAX_ATTEMPTS: usize = 4;
    let mut bytes = [0_u8; 8];
    for _ in 0..MAX_ATTEMPTS {
        getrandom::fill(&mut bytes).map_err(|_| {
            RelayLocatorIssueError::Protocol(
                op_collab_relay_protocol::RelayProtocolError::RandomUnavailable,
            )
        })?;
        if let Some(generation) = NonZeroU64::new(u64::from_be_bytes(bytes)) {
            bytes.zeroize();
            return Ok(generation);
        }
    }
    bytes.zeroize();
    Err(RelayLocatorIssueError::InvalidRandomGeneration)
}

fn unix_now() -> Result<u64, RelayLocatorIssueError> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| RelayLocatorIssueError::ClockBeforeUnixEpoch)?
        .as_secs();
    if now == 0 {
        return Err(RelayLocatorIssueError::ZeroCurrentTime);
    }
    Ok(now)
}

fn read_array<const N: usize>(
    input: &[u8],
    offset: usize,
) -> Result<[u8; N], RelayLocatorIssueError> {
    input
        .get(offset..offset + N)
        .and_then(|slice| slice.try_into().ok())
        .ok_or(RelayLocatorIssueError::InvalidRequestLength {
            actual: input.len(),
            expected: OWNER_PUBLISH_REQUEST_BYTES,
        })
}
