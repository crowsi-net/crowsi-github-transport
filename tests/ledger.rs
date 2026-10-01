use crowsi_github_transport::{ExecutionLedger, ExecutionStart};
use crowsi_provider_egress_contracts::GitHubEgressReceiptV1;

#[test]
fn prepared_is_ambiguous_completed_is_exact_and_substitution_fails_closed() {
    let directory = tempfile::tempdir().expect("ledger directory");
    let ledger = ExecutionLedger::new(directory.path().join("ledger.json"));
    assert_eq!(
        ledger.begin("authorization-1", "digest-1").expect("begin"),
        ExecutionStart::Execute
    );
    assert_eq!(
        ledger
            .begin("authorization-1", "digest-1")
            .expect_err("ambiguous")
            .0,
        "provider-outcome-unknown"
    );
    assert_eq!(
        ledger
            .begin("authorization-1", "digest-2")
            .expect_err("substitution")
            .0,
        "authorization-replay"
    );
    let receipt = receipt();
    ledger
        .complete("authorization-1", "digest-1", receipt.clone())
        .expect("complete");
    assert_eq!(
        ledger.begin("authorization-1", "digest-1").expect("replay"),
        ExecutionStart::Completed(receipt)
    );
    assert_eq!(
        ledger
            .begin("authorization-1", "digest-2")
            .expect_err("completed substitution")
            .0,
        "authorization-replay"
    );
}

fn receipt() -> GitHubEgressReceiptV1 {
    GitHubEgressReceiptV1 {
        schema: "crowsi://provider-egress/github-receipt/v1".into(),
        authorization_id: "authorization-1".into(),
        authorization_digest_sha256: "sha256:authorization".into(),
        api_request_digest_sha256: "sha256:request".into(),
        provider_request_id: "github-request-1".into(),
        remote_reference: "github://example/repository".into(),
        remote_digest_sha256: Some("sha256:remote".into()),
        completed_at_epoch_s: 100,
    }
}
