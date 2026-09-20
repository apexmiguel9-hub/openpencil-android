#[path = "../prebuilt_link_compat.rs"]
mod prebuilt_link_compat;

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use prebuilt_link_compat::{stage_archive_for_rust_host, LinkCompatError};
use sha2::{Digest, Sha256};

const HOST_PERSONALITY: &[u8] = b"rust_eh_personality";
const PRIVATE_PERSONALITY: &[u8] = b"rust_eh_personalitx";

static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(1);

struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        let unique = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "op-auth-link-compat-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn namespaces_personality_in_every_elf_and_coff_prebuilt() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let targets = [
        ("x86_64-unknown-linux-gnu", "libop_auth.a"),
        ("aarch64-unknown-linux-gnu", "libop_auth.a"),
        ("x86_64-pc-windows-msvc", "op_auth.lib"),
        ("aarch64-pc-windows-msvc", "op_auth.lib"),
    ];

    for (target, artifact) in targets {
        let source = manifest_dir.join("prebuilt").join(target).join(artifact);
        let original = fs::read(&source).unwrap();
        let temp = TempDir::new();
        let staged = temp.0.join(artifact);
        let expected_sha256 = Sha256::digest(&original).into();
        let report = stage_archive_for_rust_host(&source, &staged, Some(expected_sha256)).unwrap();
        let staged_bytes = fs::read(&staged).unwrap();

        // The provenance-checked source is never mutated in place, and the host
        // personality symbol must never survive into the staged copy.
        assert_eq!(
            report.existing_private_occurrences, 0,
            "{target} unexpectedly already contains the private symbol"
        );
        assert_eq!(fs::read(&source).unwrap(), original);
        assert_eq!(staged_bytes.len(), original.len());
        assert!(!contains(&staged_bytes, HOST_PERSONALITY));

        // Hardened Mach-O/ELF archives localize `rust_eh_personality`, so its
        // name is already absent — nothing to rename, and no clash with the
        // host toolchain. The MSVC COFF pass-through still carries it, and the
        // shim must namespace every occurrence.
        if contains(&original, HOST_PERSONALITY) {
            assert!(
                report.renamed_occurrences > 0,
                "{target} carries the host personality and must be namespaced"
            );
            assert!(contains(&staged_bytes, PRIVATE_PERSONALITY));
        } else {
            assert_eq!(
                report.renamed_occurrences, 0,
                "{target} has no personality symbol and must be copied unchanged"
            );
            assert_eq!(staged_bytes, original);
        }
    }
}

#[test]
fn already_namespaced_archive_is_copied_without_further_changes() {
    // Use an archive that actually carries the host personality so the first
    // staging namespaces it and the second sees an already-private archive.
    // Hardened ELF archives localize the symbol away, so the MSVC COFF
    // pass-through is the archive that still exercises this idempotency path.
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let source = manifest_dir
        .join("prebuilt")
        .join("x86_64-pc-windows-msvc")
        .join("op_auth.lib");
    let first_temp = TempDir::new();
    let first = first_temp.0.join("op_auth.lib");
    stage_archive_for_rust_host(&source, &first, None).unwrap();

    let second_temp = TempDir::new();
    let second = second_temp.0.join("op_auth.lib");
    let report = stage_archive_for_rust_host(&first, &second, None).unwrap();
    assert_eq!(report.renamed_occurrences, 0);
    assert!(report.existing_private_occurrences > 0);
    assert_eq!(fs::read(first).unwrap(), fs::read(second).unwrap());
}

#[test]
fn rejects_malformed_or_conflicting_archives_without_output() {
    let temp = TempDir::new();
    let malformed = temp.0.join("malformed.a");
    fs::write(&malformed, b"not an archive").unwrap();
    let destination = temp.0.join("staged.a");
    assert!(matches!(
        stage_archive_for_rust_host(&malformed, &destination, None),
        Err(LinkCompatError::InvalidArchive(_))
    ));
    assert!(!destination.exists());

    let conflicting = temp.0.join("conflicting.a");
    fs::write(
        &conflicting,
        archive_with_member([HOST_PERSONALITY, b"\0", PRIVATE_PERSONALITY, b"\0"].concat()),
    )
    .unwrap();
    assert!(matches!(
        stage_archive_for_rust_host(&conflicting, &destination, None),
        Err(LinkCompatError::ConflictingPersonalitySymbols)
    ));
    assert!(!destination.exists());
}

#[test]
fn refuses_to_rewrite_the_source_path() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let source = manifest_dir
        .join("prebuilt")
        .join("x86_64-unknown-linux-gnu")
        .join("libop_auth.a");
    assert!(matches!(
        stage_archive_for_rust_host(&source, &source, None),
        Err(LinkCompatError::SourceEqualsDestination)
    ));
}

#[test]
fn rejects_source_bytes_that_do_not_match_validated_digest() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let source = manifest_dir
        .join("prebuilt")
        .join("x86_64-unknown-linux-gnu")
        .join("libop_auth.a");
    let temp = TempDir::new();
    let destination = temp.0.join("libop_auth.a");
    assert!(matches!(
        stage_archive_for_rust_host(&source, &destination, Some([0_u8; 32])),
        Err(LinkCompatError::SourceDigestMismatch)
    ));
    assert!(!destination.exists());
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|candidate| candidate == needle)
}

fn archive_with_member(mut payload: Vec<u8>) -> Vec<u8> {
    let payload_len = payload.len();
    let mut archive = b"!<arch>\n".to_vec();
    let header = format!(
        "{:<16}{:<12}{:<6}{:<6}{:<8}{:<10}`\n",
        "fixture.o/", 0, 0, 0, 0, payload_len
    );
    assert_eq!(header.len(), 60);
    archive.extend_from_slice(header.as_bytes());
    archive.append(&mut payload);
    if !payload_len.is_multiple_of(2) {
        archive.push(b'\n');
    }
    archive
}
