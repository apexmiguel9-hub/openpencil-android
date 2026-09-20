use std::fmt;
use std::num::NonZeroU64;

use zeroize::Zeroize;

use crate::RelayProtocolError;

const ROUTE_MAP_KEY_CONTEXT: &str = "openpencil/op-collab-relay-protocol/route-map-key/v1";

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct RouteId([u8; 16]);

impl RouteId {
    pub fn new(bytes: [u8; 16]) -> Result<Self, RelayProtocolError> {
        if bytes.iter().all(|byte| *byte == 0) {
            return Err(RelayProtocolError::ZeroRouteId);
        }
        Ok(Self(bytes))
    }

    pub const fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }

    #[cfg(feature = "random")]
    pub fn generate() -> Result<Self, RelayProtocolError> {
        let mut bytes = [0_u8; 16];
        getrandom::fill(&mut bytes).map_err(|_| RelayProtocolError::RandomUnavailable)?;
        Self::new(bytes)
    }
}

impl fmt::Debug for RouteId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("RouteId([REDACTED])")
    }
}

#[derive(PartialEq, Eq)]
pub struct RouteCapability([u8; 32]);

impl RouteCapability {
    pub fn new(bytes: [u8; 32]) -> Result<Self, RelayProtocolError> {
        if bytes.iter().all(|byte| *byte == 0) {
            return Err(RelayProtocolError::ZeroRouteCapability);
        }
        Ok(Self(bytes))
    }

    pub(crate) const fn secret_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    #[cfg(feature = "random")]
    pub fn generate() -> Result<Self, RelayProtocolError> {
        let mut bytes = [0_u8; 32];
        getrandom::fill(&mut bytes).map_err(|_| RelayProtocolError::RandomUnavailable)?;
        Self::new(bytes)
    }
}

impl Clone for RouteCapability {
    fn clone(&self) -> Self {
        Self(self.0)
    }
}

impl Drop for RouteCapability {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

impl fmt::Debug for RouteCapability {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("RouteCapability([REDACTED])")
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct OwnerNoiseStatic([u8; 32]);

impl OwnerNoiseStatic {
    pub fn new(bytes: [u8; 32]) -> Result<Self, RelayProtocolError> {
        if bytes.iter().all(|byte| *byte == 0) {
            return Err(RelayProtocolError::ZeroOwnerNoiseStatic);
        }
        Ok(Self(bytes))
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Debug for OwnerNoiseStatic {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("OwnerNoiseStatic([REDACTED])")
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct CallerDeviceDhPublic([u8; 32]);

impl CallerDeviceDhPublic {
    pub fn new(bytes: [u8; 32]) -> Result<Self, RelayProtocolError> {
        if bytes.iter().all(|byte| *byte == 0) {
            return Err(RelayProtocolError::ZeroCallerDeviceDhPublic);
        }
        Ok(Self(bytes))
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Debug for CallerDeviceDhPublic {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("CallerDeviceDhPublic([REDACTED])")
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct RouteMapKey([u8; 32]);

impl RouteMapKey {
    pub fn derive(
        route_id: &RouteId,
        generation: NonZeroU64,
        capability: &RouteCapability,
    ) -> Self {
        let mut hasher = blake3::Hasher::new_derive_key(ROUTE_MAP_KEY_CONTEXT);
        hasher.update(route_id.as_bytes());
        hasher.update(&generation.get().to_be_bytes());
        hasher.update(capability.secret_bytes());
        Self(*hasher.finalize().as_bytes())
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Debug for RouteMapKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("RouteMapKey([REDACTED])")
    }
}
