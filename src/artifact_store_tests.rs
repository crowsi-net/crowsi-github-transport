use sha2::{Digest, Sha256};

use crate::artifact_store::ArtifactStore;

#[test]
fn exact_content_address_is_required_and_symlinks_are_rejected() {
    let root = tempfile::tempdir().expect("artifact root");
    let root_path = root.path().canonicalize().expect("canonical root");
    let bytes = br#"{"schema":"zixcel://github/text-artifact/v1","text":"body"}"#;
    let digest = hex::encode(Sha256::digest(bytes));
    let path = root_path.join(format!("{digest}.json"));
    std::fs::write(&path, bytes).expect("write artifact");
    let store = ArtifactStore::new(root_path.to_str().expect("root text")).expect("store");
    assert_eq!(
        store.read(&format!("sha256:{digest}")).expect("read"),
        bytes
    );
    std::fs::write(&path, b"changed").expect("tamper");
    assert_eq!(
        store
            .read(&format!("sha256:{digest}"))
            .expect_err("digest")
            .0,
        "artifact-digest"
    );

    let linked_bytes = b"linked";
    let linked_digest = hex::encode(Sha256::digest(linked_bytes));
    let target = root_path.join("target.json");
    std::fs::write(&target, linked_bytes).expect("target");
    std::os::unix::fs::symlink(&target, root_path.join(format!("{linked_digest}.json")))
        .expect("symlink");
    assert_eq!(
        store
            .read(&format!("sha256:{linked_digest}"))
            .expect_err("linked")
            .0,
        "artifact"
    );
}
