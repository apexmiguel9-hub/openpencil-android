#[cfg(unix)]
use std::{
    fmt,
    io::{Read as _, Write as _},
    path::{Component, Path, PathBuf},
    time::Duration,
};

#[cfg(unix)]
use op_collab_relay_control_plane::{RelayLocatorSigner, RelayLocatorSignerError};
use op_collab_relay_protocol::LOCATOR_CANONICAL_SIGNING_BYTES;
#[cfg(unix)]
use op_collab_relay_protocol::{LocatorKeyId, LocatorSignature, MAX_LOCATOR_KEY_ID_BYTES};

pub const HSM_SIGNING_PROTOCOL_VERSION: u8 = 1;
pub const HSM_SIGN_REQUEST_BYTES: usize = 4 + 1 + 1 + 1 + 64 + LOCATOR_CANONICAL_SIGNING_BYTES;
pub const HSM_SIGN_RESPONSE_BYTES: usize = 4 + 1 + 1 + 64;
#[cfg(unix)]
pub const MAX_HSM_SOCKET_PATH_BYTES: usize = 100;
#[cfg(unix)]
pub const MAX_HSM_TIMEOUT: Duration = Duration::from_secs(5);

#[cfg(unix)]
const REQUEST_MAGIC: &[u8; 4] = b"OPLS";
#[cfg(unix)]
const RESPONSE_MAGIC: &[u8; 4] = b"OPLR";
#[cfg(unix)]
const SIGN_OPERATION: u8 = 1;
#[cfg(unix)]
const STATUS_OK: u8 = 0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExpectedUnixPeer {
    pub uid: u32,
    pub gid: u32,
}

#[cfg(unix)]
pub struct UnixHsmRelayLocatorSigner {
    socket_path: PathBuf,
    key_id: LocatorKeyId,
    expected_peer: ExpectedUnixPeer,
    timeout: Duration,
}

#[cfg(unix)]
impl UnixHsmRelayLocatorSigner {
    pub fn new(
        socket_path: impl Into<PathBuf>,
        key_id: LocatorKeyId,
        expected_peer: ExpectedUnixPeer,
        timeout: Duration,
    ) -> Result<Self, HsmSignerConfigError> {
        let socket_path = socket_path.into();
        validate_socket_path_shape(&socket_path)?;
        if timeout.is_zero() || timeout > MAX_HSM_TIMEOUT {
            return Err(HsmSignerConfigError::InvalidTimeout);
        }
        Ok(Self {
            socket_path,
            key_id,
            expected_peer,
            timeout,
        })
    }

    pub fn validate_socket(&self) -> Result<(), HsmSignerError> {
        validate_socket_source(&self.socket_path, self.expected_peer)
    }

    fn request_signature(
        &self,
        canonical: &[u8; LOCATOR_CANONICAL_SIGNING_BYTES],
    ) -> Result<LocatorSignature, HsmSignerError> {
        use socket2::{Domain, SockAddr, Socket, Type};
        use std::{net::Shutdown, os::unix::net::UnixStream};

        validate_socket_source(&self.socket_path, self.expected_peer)?;
        let address = SockAddr::unix(&self.socket_path).map_err(|_| HsmSignerError::Unavailable)?;
        let socket = Socket::new(Domain::UNIX, Type::STREAM, None)
            .map_err(|_| HsmSignerError::Unavailable)?;
        socket
            .connect_timeout(&address, self.timeout)
            .map_err(|_| HsmSignerError::Unavailable)?;
        socket
            .set_read_timeout(Some(self.timeout))
            .map_err(|_| HsmSignerError::Unavailable)?;
        socket
            .set_write_timeout(Some(self.timeout))
            .map_err(|_| HsmSignerError::Unavailable)?;
        let mut stream: UnixStream = socket.into();
        require_peer_identity(&stream, self.expected_peer)?;

        let request = encode_request(&self.key_id, canonical);
        stream
            .write_all(&request)
            .map_err(|_| HsmSignerError::Unavailable)?;
        stream
            .shutdown(Shutdown::Write)
            .map_err(|_| HsmSignerError::Unavailable)?;
        let mut response = Vec::with_capacity(HSM_SIGN_RESPONSE_BYTES + 1);
        stream
            .take((HSM_SIGN_RESPONSE_BYTES + 1) as u64)
            .read_to_end(&mut response)
            .map_err(|_| HsmSignerError::Unavailable)?;
        decode_response(&response)
    }
}

#[cfg(unix)]
impl RelayLocatorSigner for UnixHsmRelayLocatorSigner {
    fn active_key_id(&self) -> Result<LocatorKeyId, RelayLocatorSignerError> {
        Ok(self.key_id.clone())
    }

    fn sign(
        &self,
        key_id: &LocatorKeyId,
        canonical_signing_bytes: &[u8; LOCATOR_CANONICAL_SIGNING_BYTES],
    ) -> Result<LocatorSignature, RelayLocatorSignerError> {
        if key_id != &self.key_id {
            return Err(RelayLocatorSignerError::Rejected);
        }
        self.request_signature(canonical_signing_bytes)
            .map_err(|error| match error {
                HsmSignerError::Rejected => RelayLocatorSignerError::Rejected,
                HsmSignerError::Unavailable | HsmSignerError::InvalidResponse => {
                    RelayLocatorSignerError::Unavailable
                }
            })
    }
}

#[cfg(unix)]
impl fmt::Debug for UnixHsmRelayLocatorSigner {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("UnixHsmRelayLocatorSigner")
            .field("socket_path", &"[REDACTED]")
            .field("key_id", &self.key_id)
            .field("expected_peer", &self.expected_peer)
            .field("timeout", &self.timeout)
            .finish()
    }
}

#[derive(Debug, thiserror::Error, Clone, Copy, PartialEq, Eq)]
pub enum HsmSignerConfigError {
    #[error("HSM socket path must be an absolute, bounded filesystem path")]
    InvalidSocketPath,
    #[error("HSM operation timeout is outside the allowed range")]
    InvalidTimeout,
}

#[derive(Debug, thiserror::Error, Clone, Copy, PartialEq, Eq)]
pub enum HsmSignerError {
    #[error("HSM signing service is unavailable")]
    Unavailable,
    #[error("HSM signing request was rejected")]
    Rejected,
    #[error("HSM signing service returned an invalid response")]
    InvalidResponse,
}

#[cfg(unix)]
fn validate_socket_path_shape(path: &Path) -> Result<(), HsmSignerConfigError> {
    use std::os::unix::ffi::OsStrExt as _;

    if !path.is_absolute()
        || path.as_os_str().as_bytes().is_empty()
        || path.as_os_str().as_bytes().len() > MAX_HSM_SOCKET_PATH_BYTES
        || path
            .components()
            .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(HsmSignerConfigError::InvalidSocketPath);
    }
    Ok(())
}

#[cfg(unix)]
fn validate_socket_source(
    path: &Path,
    expected_peer: ExpectedUnixPeer,
) -> Result<(), HsmSignerError> {
    use std::os::unix::fs::{FileTypeExt as _, MetadataExt as _};

    let mut current = PathBuf::new();
    for component in path.components() {
        current.push(component.as_os_str());
        let metadata =
            std::fs::symlink_metadata(&current).map_err(|_| HsmSignerError::Unavailable)?;
        if metadata.file_type().is_symlink() {
            return Err(HsmSignerError::Rejected);
        }
    }
    let metadata = std::fs::symlink_metadata(path).map_err(|_| HsmSignerError::Unavailable)?;
    if !metadata.file_type().is_socket()
        || metadata.uid() != expected_peer.uid
        || metadata.gid() != expected_peer.gid
        || metadata.mode() & 0o002 != 0
    {
        return Err(HsmSignerError::Rejected);
    }
    Ok(())
}

#[cfg(unix)]
fn encode_request(
    key_id: &LocatorKeyId,
    canonical: &[u8; LOCATOR_CANONICAL_SIGNING_BYTES],
) -> [u8; HSM_SIGN_REQUEST_BYTES] {
    let mut output = [0_u8; HSM_SIGN_REQUEST_BYTES];
    output[..4].copy_from_slice(REQUEST_MAGIC);
    output[4] = HSM_SIGNING_PROTOCOL_VERSION;
    output[5] = SIGN_OPERATION;
    output[6] = key_id.as_str().len() as u8;
    output[7..7 + key_id.as_str().len()].copy_from_slice(key_id.as_str().as_bytes());
    // The key-id field is a fixed 64-byte slot starting at offset 7. The write
    // above and the canonical offset below are only in bounds while the longest
    // protocol key id fits that slot exactly.
    const _: () = assert!(MAX_LOCATOR_KEY_ID_BYTES == 64);
    let canonical_offset = 7 + 64;
    output[canonical_offset..].copy_from_slice(canonical);
    output
}

#[cfg(unix)]
fn decode_response(response: &[u8]) -> Result<LocatorSignature, HsmSignerError> {
    if response.len() != HSM_SIGN_RESPONSE_BYTES
        || &response[..4] != RESPONSE_MAGIC
        || response[4] != HSM_SIGNING_PROTOCOL_VERSION
    {
        return Err(HsmSignerError::InvalidResponse);
    }
    match response[5] {
        STATUS_OK => LocatorSignature::new(
            response[6..]
                .try_into()
                .map_err(|_| HsmSignerError::InvalidResponse)?,
        )
        .map_err(|_| HsmSignerError::InvalidResponse),
        1 => Err(HsmSignerError::Rejected),
        _ => Err(HsmSignerError::InvalidResponse),
    }
}

#[cfg(all(unix, target_os = "linux"))]
fn require_peer_identity(
    stream: &std::os::unix::net::UnixStream,
    expected: ExpectedUnixPeer,
) -> Result<(), HsmSignerError> {
    use std::{mem::size_of, os::fd::AsRawFd as _};

    let mut credentials = libc::ucred {
        pid: 0,
        uid: 0,
        gid: 0,
    };
    let mut length = size_of::<libc::ucred>() as libc::socklen_t;
    let result = unsafe {
        libc::getsockopt(
            stream.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            (&mut credentials as *mut libc::ucred).cast(),
            &mut length,
        )
    };
    if result != 0
        || length as usize != size_of::<libc::ucred>()
        || credentials.uid != expected.uid
        || credentials.gid != expected.gid
    {
        return Err(HsmSignerError::Rejected);
    }
    Ok(())
}

#[cfg(all(
    unix,
    any(
        target_os = "macos",
        target_os = "ios",
        target_os = "freebsd",
        target_os = "dragonfly",
        target_os = "openbsd",
        target_os = "netbsd"
    )
))]
fn require_peer_identity(
    stream: &std::os::unix::net::UnixStream,
    expected: ExpectedUnixPeer,
) -> Result<(), HsmSignerError> {
    use std::os::fd::AsRawFd as _;

    let mut uid = 0;
    let mut gid = 0;
    let result = unsafe { libc::getpeereid(stream.as_raw_fd(), &mut uid, &mut gid) };
    if result != 0 || uid != expected.uid || gid != expected.gid {
        return Err(HsmSignerError::Rejected);
    }
    Ok(())
}

#[cfg(all(
    unix,
    not(any(
        target_os = "linux",
        target_os = "macos",
        target_os = "ios",
        target_os = "freebsd",
        target_os = "dragonfly",
        target_os = "openbsd",
        target_os = "netbsd"
    ))
))]
fn require_peer_identity(
    _stream: &std::os::unix::net::UnixStream,
    _expected: ExpectedUnixPeer,
) -> Result<(), HsmSignerError> {
    Err(HsmSignerError::Unavailable)
}
