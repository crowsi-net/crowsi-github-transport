use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::Read;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::Path;

use crate::TransportError;

pub(crate) fn verify_executable(path: &str, expected: &str) -> Result<(), TransportError> {
    let path = Path::new(path);
    let link = path
        .symlink_metadata()
        .map_err(|_| TransportError("executable"))?;
    if link.file_type().is_symlink() || !link.is_file() || link.permissions().mode() & 0o022 != 0 {
        return Err(TransportError("executable"));
    }
    let canonical = path
        .canonicalize()
        .map_err(|_| TransportError("executable"))?;
    if canonical != path {
        return Err(TransportError("executable"));
    }
    let mut file = File::open(path).map_err(|_| TransportError("executable"))?;
    let before = file.metadata().map_err(|_| TransportError("executable"))?;
    let mut hash = Sha256::new();
    let mut buffer = [0_u8; 16_384];
    loop {
        let count = file
            .read(&mut buffer)
            .map_err(|_| TransportError("executable"))?;
        if count == 0 {
            break;
        }
        hash.update(&buffer[..count]);
    }
    let after = file.metadata().map_err(|_| TransportError("executable"))?;
    if before.dev() != after.dev() || before.ino() != after.ino() || before.len() != after.len() {
        return Err(TransportError("executable"));
    }
    let actual = format!("sha256:{}", hex::encode(hash.finalize()));
    if actual != expected {
        return Err(TransportError("executable"));
    }
    Ok(())
}
