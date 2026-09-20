use std::{
    fs::{File, OpenOptions},
    io::Read as _,
    os::unix::{fs::MetadataExt as _, fs::OpenOptionsExt as _},
    path::{Path, PathBuf},
};

use zeroize::Zeroizing;

use crate::{SignerError, SignerResult};

const MAX_CONFIG_BYTES: usize = 16 * 1024;
const MAX_PIN_BYTES: usize = 128;
const MIN_PIN_BYTES: usize = 8;

pub fn read_config(path: &Path) -> SignerResult<Vec<u8>> {
    let (mut file, metadata) = open_no_follow(path)?;
    let effective_uid = unsafe { libc::geteuid() };
    if !metadata.file_type().is_file()
        || metadata.nlink() != 1
        || metadata.uid() != 0 && metadata.uid() != effective_uid
        || metadata.mode() & 0o022 != 0
    {
        return Err(secure_error(
            "config must be a single-link regular file owned by root/current uid and not writable by group/other",
        ));
    }
    read_bounded(&mut file, MAX_CONFIG_BYTES)
}

pub fn read_pin(path: &Path) -> SignerResult<Zeroizing<Vec<u8>>> {
    let (mut file, metadata) = open_no_follow(path)?;
    let effective_uid = unsafe { libc::geteuid() };
    if !metadata.file_type().is_file()
        || metadata.nlink() != 1
        || metadata.uid() != effective_uid
        || metadata.mode() & 0o777 != 0o400
    {
        return Err(secure_error(
            "PIN file must be a single-link regular file owned by the signer uid with mode 0400",
        ));
    }
    let bytes = Zeroizing::new(read_bounded(&mut file, MAX_PIN_BYTES)?);
    if !(MIN_PIN_BYTES..=MAX_PIN_BYTES).contains(&bytes.len())
        || !bytes.iter().all(|byte| (0x21..=0x7e).contains(byte))
    {
        return Err(secure_error(
            "PIN must be 8-128 printable non-whitespace ASCII bytes without a trailing newline",
        ));
    }
    Ok(bytes)
}

fn open_no_follow(path: &Path) -> SignerResult<(File, std::fs::Metadata)> {
    validate_components(path)?;
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_CLOEXEC | libc::O_NOFOLLOW)
        .open(path)
        .map_err(|_| secure_error("secure file could not be opened"))?;
    let metadata = file
        .metadata()
        .map_err(|_| secure_error("secure file metadata is unavailable"))?;
    Ok((file, metadata))
}

fn validate_components(path: &Path) -> SignerResult<()> {
    if !path.is_absolute() {
        return Err(secure_error("secure file path must be absolute"));
    }
    let mut current = PathBuf::new();
    for component in path.components() {
        current.push(component.as_os_str());
        let metadata = std::fs::symlink_metadata(&current)
            .map_err(|_| secure_error("secure path component is unavailable"))?;
        if metadata.file_type().is_symlink() {
            return Err(secure_error("secure path must not contain symlinks"));
        }
    }
    Ok(())
}

fn read_bounded(file: &mut File, maximum: usize) -> SignerResult<Vec<u8>> {
    let mut bytes = Vec::with_capacity(maximum.min(4096));
    file.take((maximum + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| secure_error("secure file could not be read"))?;
    if bytes.len() > maximum {
        return Err(secure_error("secure file exceeds its size limit"));
    }
    Ok(bytes)
}

fn secure_error(message: impl Into<String>) -> SignerError {
    SignerError::SecureFile(message.into())
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::PermissionsExt as _;

    use super::*;

    #[test]
    fn pin_requires_exact_permissions_and_no_newline() {
        let directory = tempfile::Builder::new()
            .prefix("pin-test-")
            .tempdir_in(env!("CARGO_MANIFEST_DIR"))
            .unwrap();
        let path = directory.path().join("pin");
        std::fs::write(&path, b"test-pin-123").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o400)).unwrap();
        assert_eq!(read_pin(&path).unwrap().as_slice(), b"test-pin-123");

        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        assert!(read_pin(&path).is_err());
    }
}
