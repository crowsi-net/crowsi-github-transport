use crowsi_provider_egress_contracts::{
    GitHubEgressCommandV1, GitHubEgressMode, GitHubEgressReceiptV1, GitHubVerificationReceiptV1,
    github_connection_ref_digest, validate_github_command,
};
use zixcel_github::{
    GitHubApiRequest, GitHubBackendReceipt, parse_api_request_json, validate_api_request,
};

use crate::{GitHubTransportConfigV1, TransportError};

pub trait GitHubProvider {
    /// Executes one request after Crowsi authorization validation.
    ///
    /// # Errors
    ///
    /// Returns an error without creating a provider receipt if execution fails.
    fn perform(
        &mut self,
        request: &GitHubApiRequest,
    ) -> Result<GitHubBackendReceipt, TransportError>;
}

/// Verifies or executes one exact GitHub command.
///
/// # Errors
///
/// Rejects invalid signatures, time, document digests, connection binding,
/// permission binding, provider responses, or execution in verify mode.
pub fn execute_command<P: GitHubProvider>(
    value: &GitHubEgressCommandV1,
    config: &GitHubTransportConfigV1,
    provider: &mut P,
) -> Result<serde_json::Value, TransportError> {
    validate_github_command(value).map_err(TransportError)?;
    let authorization_digest = crate::authorization::verify(value, &config.trust)?;
    let request = parse_api_request_json(value.api_request_json.as_bytes())
        .map_err(|_| TransportError("api-request"))?;
    let validation = validate_api_request(&request).map_err(|_| TransportError("api-request"))?;
    let auth = &value.authorization;
    if github_connection_ref_digest(&request.connection_ref) != auth.connection_ref_digest_sha256
        || validation.required_permission.resource != auth.permission_resource
        || validation.required_permission.operation != auth.permission_operation
    {
        return Err(TransportError("authorization-binding"));
    }
    if !config
        .connector_configs
        .iter()
        .any(|policy| policy.authorize_request(&request).is_ok())
    {
        return Err(TransportError("connector-policy"));
    }
    match value.mode {
        GitHubEgressMode::Verify => serde_json::to_value(GitHubVerificationReceiptV1 {
            schema: "crowsi://provider-egress/github-verification-receipt/v1".into(),
            authorization_id: auth.authorization_id.clone(),
            authorization_digest_sha256: authorization_digest,
            grant_digest_sha256: auth.grant_digest_sha256.clone(),
            invocation_digest_sha256: auth.invocation_digest_sha256.clone(),
            api_request_digest_sha256: auth.api_request_digest_sha256.clone(),
            expires_at_epoch_s: auth.expires_at_epoch_s,
        })
        .map_err(|_| TransportError("receipt")),
        GitHubEgressMode::Execute => {
            let receipt = provider.perform(&request)?;
            validate_backend(&receipt)?;
            serde_json::to_value(GitHubEgressReceiptV1 {
                schema: "crowsi://provider-egress/github-receipt/v1".into(),
                authorization_id: auth.authorization_id.clone(),
                authorization_digest_sha256: authorization_digest,
                api_request_digest_sha256: auth.api_request_digest_sha256.clone(),
                provider_request_id: receipt.provider_request_id,
                remote_reference: receipt.remote_reference,
                remote_digest_sha256: receipt.remote_digest_sha256,
                completed_at_epoch_s: value.now_epoch_s,
            })
            .map_err(|_| TransportError("receipt"))
        }
    }
}

fn validate_backend(value: &GitHubBackendReceipt) -> Result<(), TransportError> {
    if value.provider_request_id.is_empty()
        || value.provider_request_id.len() > 128
        || value.remote_reference.is_empty()
        || value.remote_reference.len() > 512
        || value
            .remote_digest_sha256
            .as_ref()
            .is_some_and(|item| item.len() != 71 || !item.starts_with("sha256:"))
    {
        return Err(TransportError("provider-receipt"));
    }
    Ok(())
}
