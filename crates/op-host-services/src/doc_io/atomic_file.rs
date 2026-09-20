//! Sibling-temp creation and atomic destination replacement for `.op` saves.
//!
//! Public: op-host-desktop's `mcp_config_io` reuses these primitives for its
//! crash-safe MCP-integration config writes instead of keeping its own copy
//! of the Windows ReplaceFileW/MoveFileExW FFI.

use std::path::{Path, PathBuf};

use super::DocIoError;

/// Create a unique sibling with `create_new`, so overlapping saves and stale
/// files from a recycled PID can never truncate one another.
pub fn create_sibling_temp(path: &Path) -> Result<(PathBuf, std::fs::File), DocIoError> {
    for _ in 0..128 {
        let candidate = unique_sibling_temp_path(path);
        match std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&candidate)
        {
            Ok(file) => return Ok((candidate, file)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(DocIoError::CreateTemp {
                    path: candidate.display().to_string(),
                    detail: error.to_string(),
                })
            }
        }
    }
    Err(DocIoError::TempAllocExhausted {
        path: path.display().to_string(),
    })
}

fn unique_sibling_temp_path(path: &Path) -> PathBuf {
    static NEXT_TEMP: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let sequence = NEXT_TEMP.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let pid = std::process::id();
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("document.op");
    path.with_file_name(format!(".{file_name}.{pid}.{sequence}.tmp"))
}

#[cfg(not(windows))]
pub fn replace_file(tmp: &Path, path: &Path) -> Result<(), DocIoError> {
    std::fs::rename(tmp, path).map_err(|error| DocIoError::Replace {
        destination: path.display().to_string(),
        temp: tmp.display().to_string(),
        detail: error.to_string(),
    })
}

/// Windows `std::fs::rename` cannot replace an existing destination. These OS
/// primitives preserve the old file until the completed sibling temp commits.
#[cfg(windows)]
pub fn replace_file(tmp: &Path, path: &Path) -> Result<(), DocIoError> {
    use std::os::windows::ffi::OsStrExt;
    use std::ptr;

    // ReplaceFileW and MoveFileExW can race when background save jobs commit
    // different sibling temps to the same destination. Keep only the final
    // name swap serialized; JSON encoding and file I/O remain concurrent.
    static REPLACE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _replace_guard = REPLACE_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);

    const REPLACEFILE_IGNORE_MERGE_ERRORS: u32 = 0x2;
    const MOVEFILE_REPLACE_EXISTING: u32 = 0x1;
    const MOVEFILE_WRITE_THROUGH: u32 = 0x8;

    #[link(name = "Kernel32")]
    #[allow(non_snake_case)]
    extern "system" {
        fn ReplaceFileW(
            replaced_file_name: *const u16,
            replacement_file_name: *const u16,
            backup_file_name: *const u16,
            replace_flags: u32,
            exclude: *mut std::ffi::c_void,
            reserved: *mut std::ffi::c_void,
        ) -> i32;
        fn MoveFileExW(
            existing_file_name: *const u16,
            new_file_name: *const u16,
            flags: u32,
        ) -> i32;
    }

    fn wide(path: &Path) -> Vec<u16> {
        path.as_os_str().encode_wide().chain(Some(0)).collect()
    }

    let temporary = wide(tmp);
    let destination = wide(path);
    // SAFETY: both path buffers are NUL-terminated and remain alive for each
    // call; the optional Windows API pointer arguments are documented nullable.
    let replaced = unsafe {
        ReplaceFileW(
            destination.as_ptr(),
            temporary.as_ptr(),
            ptr::null(),
            REPLACEFILE_IGNORE_MERGE_ERRORS,
            ptr::null_mut(),
            ptr::null_mut(),
        )
    };
    if replaced != 0 {
        return Ok(());
    }

    // ReplaceFileW requires an existing destination. MoveFileExW also covers
    // first-save and destination appearance/disappearance races atomically.
    let moved = unsafe {
        MoveFileExW(
            temporary.as_ptr(),
            destination.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if moved != 0 {
        Ok(())
    } else {
        Err(DocIoError::Replace {
            destination: path.display().to_string(),
            temp: tmp.display().to_string(),
            detail: std::io::Error::last_os_error().to_string(),
        })
    }
}
