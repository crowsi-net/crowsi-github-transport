use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

use crate::TransportError;

pub(crate) struct ArtifactStore {
    root: PathBuf,
}

impl ArtifactStore {
    pub(crate) fn new(root: &str) -> Result<Self, TransportError> {
        let root = Path::new(root)
            .canonicalize()
            .map_err(|_| TransportError("artifact-root"))?;
        if !root.is_dir() {
            return Err(TransportError("artifact-root"));
        }
        Ok(Self { root })
    }

    pub(crate) fn read(&self, reference: &str) -> Result<Vec<u8>, TransportError> {
        let digest = reference
            .strip_prefix("sha256:")
            .ok_or(TransportError("artifact-ref"))?;
        if digest.len() != 64 || !digest.bytes().all(lower_hex) {
            return Err(TransportError("artifact-ref"));
        }
        let path = self.root.join(format!("{digest}.json"));
        let metadata = path
            .symlink_metadata()
            .map_err(|_| TransportError("artifact"))?;
        if metadata.file_type().is_symlink()
            || !metadata.is_file()
            || metadata.len() > 16 * 1024 * 1024
        {
            return Err(TransportError("artifact"));
        }
        let bytes = std::fs::read(&path).map_err(|_| TransportError("artifact"))?;
        if path
            .canonicalize()
            .map_err(|_| TransportError("artifact"))?
            != self.root.join(format!("{digest}.json"))
        {
            return Err(TransportError("artifact"));
        }
        let actual = hex::encode(Sha256::digest(&bytes));
        if actual != digest {
            return Err(TransportError("artifact-digest"));
        }
        Ok(bytes)
    }
}

fn lower_hex(value: u8) -> bool {
    value.is_ascii_digit() || (b'a'..=b'f').contains(&value)
}
