use std::{
    fs::{File, OpenOptions},
    io::{Read as _, Seek as _, SeekFrom, Write as _},
    os::{
        fd::AsRawFd as _,
        unix::{
            fs::{FileTypeExt as _, MetadataExt as _, OpenOptionsExt as _, PermissionsExt as _},
            net::{UnixListener, UnixStream},
        },
    },
    path::{Path, PathBuf},
    time::Duration,
};

use crate::{
    protocol::{self, REQUEST_BYTES},
    KeyStore, SignerConfig, SignerError, SignerResult,
};

pub fn serve(config: &SignerConfig, store: &KeyStore) -> SignerResult<()> {
    let (_lock, mut heartbeat, _socket_guard, listener) = bind_socket(config)?;
    listener.set_nonblocking(true)?;
    tracing::info!("OPLS signer is ready");
    let mut pulse = 0_u8;
    loop {
        write_heartbeat(&mut heartbeat, pulse)?;
        pulse = pulse.wrapping_add(1);
        match listener.accept() {
            Ok((mut stream, _)) => {
                if let Err(error) = handle_connection(config, store, &mut stream) {
                    if matches!(error, SignerError::Rejected) {
                        tracing::debug!("OPLS request rejected");
                    } else {
                        tracing::warn!(error = %error, "OPLS request failed");
                    }
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(250));
            }
            Err(error) => {
                tracing::warn!(error = %error, "OPLS accept failed");
                std::thread::sleep(Duration::from_millis(250));
            }
        }
    }
}

pub fn probe_socket(config: &SignerConfig) -> SignerResult<()> {
    validate_bound_socket(config)?;
    validate_heartbeat(config)
}

fn handle_connection(
    config: &SignerConfig,
    store: &KeyStore,
    stream: &mut UnixStream,
) -> SignerResult<()> {
    require_peer(
        stream,
        config.expected_client_uid,
        config.expected_client_gid,
    )?;
    let timeout = Duration::from_millis(config.request_timeout_ms);
    stream.set_read_timeout(Some(timeout))?;
    stream.set_write_timeout(Some(timeout))?;
    let mut request = [0_u8; REQUEST_BYTES];
    if stream.read_exact(&mut request).is_err() {
        stream.write_all(&protocol::rejected_response())?;
        return Err(SignerError::Rejected);
    }
    let mut trailing = [0_u8; 1];
    if stream.read(&mut trailing)? != 0 {
        stream.write_all(&protocol::rejected_response())?;
        return Err(SignerError::Rejected);
    }
    let decoded = match protocol::decode_request(&request, config.region, &config.active_kid) {
        Ok(value) => value,
        Err(SignerError::Rejected) => {
            stream.write_all(&protocol::rejected_response())?;
            return Err(SignerError::Rejected);
        }
        Err(error) => return Err(error),
    };
    let signature = store.sign_locator(&decoded.canonical)?;
    stream.write_all(&protocol::success_response(&signature))?;
    Ok(())
}

fn bind_socket(config: &SignerConfig) -> SignerResult<(File, File, SocketGuard, UnixListener)> {
    let effective_uid = unsafe { libc::geteuid() };
    let effective_gid = unsafe { libc::getegid() };
    if effective_uid == 0
        || effective_uid == config.expected_client_uid
        || effective_gid != config.expected_client_gid
    {
        return Err(SignerError::Config(
            "signer must run non-root under a uid distinct from the locator and their shared gid"
                .into(),
        ));
    }
    let parent = config
        .socket_path
        .parent()
        .ok_or_else(|| SignerError::Config("socket path has no parent".into()))?;
    validate_socket_parent(parent, effective_uid, effective_gid)?;
    let lock_path = parent.join("signer.lock");
    let lock = lock_exclusive(&lock_path, effective_uid)?;
    let heartbeat = open_heartbeat(&parent.join("signer.ready"), effective_uid)?;
    remove_stale_socket(config, effective_uid, effective_gid)?;
    let listener = UnixListener::bind(&config.socket_path)
        .map_err(|_| SignerError::Unavailable("could not bind signer socket".into()))?;
    std::fs::set_permissions(&config.socket_path, std::fs::Permissions::from_mode(0o660))?;
    validate_bound_socket(config)?;
    let metadata = std::fs::symlink_metadata(&config.socket_path)?;
    let guard = SocketGuard {
        path: config.socket_path.clone(),
        device: metadata.dev(),
        inode: metadata.ino(),
    };
    Ok((lock, heartbeat, guard, listener))
}

fn validate_socket_parent(path: &Path, uid: u32, gid: u32) -> SignerResult<()> {
    let mut current = PathBuf::new();
    for component in path.components() {
        current.push(component.as_os_str());
        let metadata = std::fs::symlink_metadata(&current)
            .map_err(|_| SignerError::Unavailable("socket path component is unavailable".into()))?;
        if metadata.file_type().is_symlink() {
            return Err(SignerError::Config(
                "socket path must not contain symlinks".into(),
            ));
        }
    }
    let metadata = std::fs::symlink_metadata(path)?;
    if !metadata.is_dir()
        || metadata.uid() != 0 && metadata.uid() != uid
        || metadata.mode() & 0o002 != 0
        || metadata.mode() & 0o020 != 0 && metadata.gid() != gid
        || metadata.mode() & 0o110 != 0o110
    {
        return Err(SignerError::Config(
            "socket parent ownership or permissions are unsafe".into(),
        ));
    }
    Ok(())
}

fn lock_exclusive(path: &Path, uid: u32) -> SignerResult<File> {
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .mode(0o600)
        .custom_flags(libc::O_CLOEXEC | libc::O_NOFOLLOW)
        .open(path)
        .map_err(|_| SignerError::Unavailable("signer lock could not be opened".into()))?;
    let metadata = lock.metadata()?;
    if !metadata.file_type().is_file()
        || metadata.nlink() != 1
        || metadata.uid() != uid
        || metadata.mode() & 0o777 != 0o600
    {
        return Err(SignerError::Config("signer lock is unsafe".into()));
    }
    let result = unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
    if result != 0 {
        return Err(SignerError::Unavailable(
            "another signer owns the socket lock".into(),
        ));
    }
    Ok(lock)
}

fn open_heartbeat(path: &Path, uid: u32) -> SignerResult<File> {
    let heartbeat = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .custom_flags(libc::O_CLOEXEC | libc::O_NOFOLLOW)
        .open(path)
        .map_err(|_| SignerError::Unavailable("signer heartbeat could not be opened".into()))?;
    let metadata = heartbeat.metadata()?;
    if !metadata.file_type().is_file()
        || metadata.nlink() != 1
        || metadata.uid() != uid
        || metadata.mode() & 0o777 != 0o600
    {
        return Err(SignerError::Config("signer heartbeat is unsafe".into()));
    }
    Ok(heartbeat)
}

fn write_heartbeat(heartbeat: &mut File, pulse: u8) -> SignerResult<()> {
    heartbeat.seek(SeekFrom::Start(0))?;
    heartbeat.write_all(&[pulse])?;
    heartbeat.flush()?;
    Ok(())
}

fn validate_heartbeat(config: &SignerConfig) -> SignerResult<()> {
    let path = config
        .socket_path
        .parent()
        .ok_or_else(|| SignerError::Config("socket path has no parent".into()))?
        .join("signer.ready");
    let metadata = std::fs::symlink_metadata(path)
        .map_err(|_| SignerError::Unavailable("signer heartbeat is unavailable".into()))?;
    if !metadata.file_type().is_file()
        || metadata.uid() != unsafe { libc::geteuid() }
        || metadata.mode() & 0o777 != 0o600
        || metadata
            .modified()
            .ok()
            .and_then(|modified| modified.elapsed().ok())
            .is_none_or(|age| age > Duration::from_secs(10))
    {
        return Err(SignerError::Unavailable(
            "signer serving loop heartbeat is stale".into(),
        ));
    }
    Ok(())
}

fn remove_stale_socket(config: &SignerConfig, uid: u32, gid: u32) -> SignerResult<()> {
    let metadata = match std::fs::symlink_metadata(&config.socket_path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.into()),
    };
    if !metadata.file_type().is_socket()
        || metadata.uid() != uid
        || metadata.gid() != gid
        || metadata.mode() & 0o002 != 0
    {
        return Err(SignerError::Config(
            "existing signer socket has unsafe identity or permissions".into(),
        ));
    }
    match UnixStream::connect(&config.socket_path) {
        Ok(_) => Err(SignerError::Unavailable(
            "another signer is already listening".into(),
        )),
        Err(error) if error.raw_os_error() == Some(libc::ECONNREFUSED) => {
            std::fs::remove_file(&config.socket_path)?;
            Ok(())
        }
        Err(_) => Err(SignerError::Unavailable(
            "existing signer socket could not be safely classified as stale".into(),
        )),
    }
}

fn validate_bound_socket(config: &SignerConfig) -> SignerResult<()> {
    let metadata = std::fs::symlink_metadata(&config.socket_path)
        .map_err(|_| SignerError::Unavailable("signer socket is unavailable".into()))?;
    let uid = unsafe { libc::geteuid() };
    let gid = unsafe { libc::getegid() };
    if !metadata.file_type().is_socket()
        || metadata.uid() != uid
        || metadata.gid() != gid
        || metadata.mode() & 0o777 != 0o660
    {
        return Err(SignerError::Config(
            "bound signer socket has unexpected identity or permissions".into(),
        ));
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn require_peer(stream: &UnixStream, expected_uid: u32, expected_gid: u32) -> SignerResult<()> {
    let mut credentials = libc::ucred {
        pid: 0,
        uid: 0,
        gid: 0,
    };
    let mut length = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
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
        || length as usize != std::mem::size_of::<libc::ucred>()
        || credentials.uid != expected_uid
        || credentials.gid != expected_gid
    {
        return Err(SignerError::Rejected);
    }
    Ok(())
}

#[cfg(any(
    target_os = "macos",
    target_os = "ios",
    target_os = "freebsd",
    target_os = "dragonfly",
    target_os = "openbsd",
    target_os = "netbsd"
))]
fn require_peer(stream: &UnixStream, expected_uid: u32, expected_gid: u32) -> SignerResult<()> {
    let mut uid = 0;
    let mut gid = 0;
    let result = unsafe { libc::getpeereid(stream.as_raw_fd(), &mut uid, &mut gid) };
    if result != 0 || uid != expected_uid || gid != expected_gid {
        return Err(SignerError::Rejected);
    }
    Ok(())
}

struct SocketGuard {
    path: PathBuf,
    device: u64,
    inode: u64,
}

impl Drop for SocketGuard {
    fn drop(&mut self) {
        if let Ok(metadata) = std::fs::symlink_metadata(&self.path) {
            if metadata.file_type().is_socket()
                && metadata.dev() == self.device
                && metadata.ino() == self.inode
            {
                let _ = std::fs::remove_file(&self.path);
            }
        }
    }
}
