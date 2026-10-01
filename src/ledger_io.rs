use std::fs::{OpenOptions, create_dir_all, rename};
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

use crate::TransportError;

pub(crate) fn replace(path: &Path, bytes: &[u8]) -> Result<(), TransportError> {
    let parent = path.parent().ok_or(TransportError("ledger-path"))?;
    create_dir_all(parent).map_err(|_| TransportError("ledger-write"))?;
    let mut nonce = [0_u8; 16];
    getrandom::fill(&mut nonce).map_err(|_| TransportError("entropy"))?;
    let leaf = path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or(TransportError("ledger-path"))?;
    let temporary = parent.join(format!(".{leaf}.{}.tmp", hex::encode(nonce)));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&temporary)
        .map_err(|_| TransportError("ledger-write"))?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|_| TransportError("ledger-write"))?;
    rename(&temporary, path).map_err(|_| TransportError("ledger-write"))?;
    std::fs::File::open(parent)
        .and_then(|value| value.sync_all())
        .map_err(|_| TransportError("ledger-write"))
}
