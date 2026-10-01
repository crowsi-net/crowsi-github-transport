use crowsi_credential_broker::PlatformCustodyConfigV1;
use crowsi_github_transport::{GitHubProvider, GitHubTransportConfigV1, execute_command};
use crowsi_provider_egress_contracts::{
    GitHubAuthorizationV1, GitHubEgressCommandV1, GitHubEgressMode, authorization_digest,
    github_connection_ref_digest, github_document_digest,
};
use ed25519_dalek::{Signer, SigningKey};
use sha2::Digest;
use std::collections::BTreeMap;
use zixcel_github::{GitHubApiRequest, GitHubBackendReceipt};

struct Provider {
    calls: usize,
}
impl GitHubProvider for Provider {
    fn perform(
        &mut self,
        _: &GitHubApiRequest,
    ) -> Result<GitHubBackendReceipt, crowsi_github_transport::TransportError> {
        self.calls += 1;
        Ok(GitHubBackendReceipt {
            provider_request_id: "github-request-1".into(),
            remote_reference: "github://example/repository".into(),
            remote_digest_sha256: None,
        })
    }
}

#[test]
fn signature_connection_permission_and_documents_are_exact() {
    let key = SigningKey::from_bytes(&[7_u8; 32]);
    let config = config(&key);
    let mut command = command();
    command.authorization.signature_hex = hex::encode(
        key.sign(
            authorization_digest(&command.authorization)
                .expect("digest")
                .as_bytes(),
        )
        .to_bytes(),
    );
    let mut provider = Provider { calls: 0 };
    let verify = execute_command(&command, &config, &mut provider).expect("verify");
    assert_eq!(verify["authorization_id"], "authorization-1");
    assert_eq!(provider.calls, 0);
    command.mode = GitHubEgressMode::Execute;
    let receipt = execute_command(&command, &config, &mut provider).expect("execute");
    assert_eq!(receipt["remote_reference"], "github://example/repository");
    assert_eq!(provider.calls, 1);
    command.authorization.permission_operation = "create-work-item".into();
    assert!(execute_command(&command, &config, &mut provider).is_err());
    assert_eq!(provider.calls, 1);
}

fn config(key: &SigningKey) -> GitHubTransportConfigV1 {
    let executable = std::env::current_exe().expect("test executable");
    let bytes = std::fs::read(&executable).expect("executable bytes");
    let helper_sha256 = format!("sha256:{}", hex::encode(sha2::Sha256::digest(bytes)));
    let custody = PlatformCustodyConfigV1::new(
        "crowsi.github.transport.test".into(),
        executable,
        helper_sha256,
    )
    .expect("custody config");
    let connection = "owner/zixcel-github/github-api/github-api/github-oauth-1";
    GitHubTransportConfigV1 {
        connector_configs: vec![zixcel_github::ConnectorConfig {
            schema: zixcel_github::CONFIG_SCHEMA.into(),
            config_id: "test".into(),
            connection_ref: connection.into(),
            organization: "example".into(),
            repositories: vec!["repo".into()],
            allowed_actions: vec!["create-private-repository".into()],
        }],
        schema: "crowsi://github-transport/config/v1".into(),
        trust: crowsi_github_transport::GitHubTransportTrustV1 {
            issuer: "crowsi-pa".into(),
            audience: "crowsi-github-transport".into(),
            key_id: "pa-key-1".into(),
            public_key_hex: hex::encode(key.verifying_key().as_bytes()),
            minimum_issued_at_epoch_s: 100,
        },
        custody,
        connections: BTreeMap::from([(
            github_connection_ref_digest(connection),
            connection.into(),
        )]),
        artifact_root: "/tmp".into(),
        state_path: "/tmp/crowsi-github-test.json".into(),
        curl_path: "/bin/false".into(),
        curl_sha256: format!("sha256:{}", "0".repeat(64)),
    }
}

fn command() -> GitHubEgressCommandV1 {
    let grant_json = "{}".to_owned();
    let invocation_json = "{}".to_owned();
    let api_request_json = serde_json::json!({
        "schema":"zixcel://github/api-request/v1", "request_id":"request-1",
        "operation_id":"operation-1", "connection_ref":"owner/zixcel-github/github-api/github-api/github-oauth-1",
        "owner":"example", "repository":"repo", "action": {
            "type":"create-private-repository", "default_branch":"main" }
    }).to_string();
    GitHubEgressCommandV1 {
        schema: "crowsi://provider-egress/github-command/v1".into(),
        mode: GitHubEgressMode::Verify,
        authorization: GitHubAuthorizationV1 {
            schema: "crowsi://provider-egress/github-authorization/v1".into(),
            authorization_id: "authorization-1".into(),
            issuer: "crowsi-pa".into(),
            audience: "crowsi-github-transport".into(),
            grant_digest_sha256: github_document_digest(&grant_json),
            invocation_digest_sha256: github_document_digest(&invocation_json),
            api_request_digest_sha256: github_document_digest(&api_request_json),
            connection_ref_digest_sha256: github_connection_ref_digest(
                "owner/zixcel-github/github-api/github-api/github-oauth-1",
            ),
            pairwise_subject: "subject-1".into(),
            device_ref: "device-1".into(),
            workload_id: "spiffe://crowsi.local/hat/github-operator".into(),
            grant_id: "grant-1".into(),
            proof_key_ref: "proof-key-1".into(),
            permission_resource: "github-private-repository".into(),
            permission_operation: "create-private-resource".into(),
            issued_at_epoch_s: 100,
            expires_at_epoch_s: 200,
            key_id: "pa-key-1".into(),
            signature_hex: String::new(),
        },
        grant_json,
        invocation_json,
        api_request_json,
        now_epoch_s: 150,
    }
}

#[test]
fn a_valid_signature_cannot_bypass_an_absent_or_read_only_local_policy() {
    let key = SigningKey::from_bytes(&[7_u8; 32]);
    let mut config = config(&key);
    let mut command = command();
    command.authorization.signature_hex = hex::encode(
        key.sign(
            authorization_digest(&command.authorization)
                .expect("signed command fixture")
                .as_bytes(),
        )
        .to_bytes(),
    );
    command.mode = GitHubEgressMode::Execute;
    let mut provider = Provider { calls: 0 };
    config.connector_configs[0].allowed_actions.clear();
    assert!(execute_command(&command, &config, &mut provider).is_err());
    config.connector_configs.clear();
    assert!(execute_command(&command, &config, &mut provider).is_err());
    assert_eq!(provider.calls, 0);
}
