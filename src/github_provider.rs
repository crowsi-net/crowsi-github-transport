use crowsi_credential_broker::{
    AccessRequest, Broker, LeaseBinding, PlatformCustodyStore, SecretRef,
};
use crowsi_provider_egress_contracts::{
    GitHubAuthorizationV1, GitHubEgressCommandV1, github_connection_ref_digest,
};
use std::path::PathBuf;
use std::sync::Arc;
use zixcel_github::{GitHubApiRequest, GitHubBackendReceipt};

use crate::artifact_store::ArtifactStore;
use crate::{GitHubProvider, GitHubTransportConfigV1, TransportError};

pub struct PlatformGitHubProvider {
    broker: Broker<PlatformCustodyStore>,
    reference: SecretRef,
    binding: LeaseBinding,
    curl_path: String,
    state_path: PathBuf,
    artifacts: ArtifactStore,
    connection_digest: String,
    policies: Vec<zixcel_github::ConnectorConfig>,
}

impl PlatformGitHubProvider {
    /// Creates the credential-bound provider for one signed command.
    ///
    /// # Errors
    ///
    /// Rejects an unknown connection or invalid custody/binding configuration.
    pub fn new(
        config: &GitHubTransportConfigV1,
        command: &GitHubEgressCommandV1,
    ) -> Result<Self, TransportError> {
        let auth = &command.authorization;
        let reference = config
            .connections
            .get(&auth.connection_ref_digest_sha256)
            .ok_or(TransportError("connection"))?;
        let reference =
            SecretRef::parse_canonical(reference).map_err(|_| TransportError("connection"))?;
        let store =
            PlatformCustodyStore::new(&config.custody).map_err(|_| TransportError("custody"))?;
        let broker = Broker::new(Arc::new(store)).map_err(|_| TransportError("custody"))?;
        Ok(Self {
            broker,
            reference,
            binding: binding(auth)?,
            curl_path: config.curl_path.clone(),
            state_path: config.state_path.clone().into(),
            artifacts: ArtifactStore::new(&config.artifact_root)?,
            connection_digest: auth.connection_ref_digest_sha256.clone(),
            policies: config.connector_configs.clone(),
        })
    }

    fn execute(&self, request: &GitHubApiRequest) -> Result<GitHubBackendReceipt, TransportError> {
        if github_connection_ref_digest(&request.connection_ref) != self.connection_digest {
            return Err(TransportError("connection"));
        }
        // Evaluate the local ceiling before credential issuance or any network request.
        if !self
            .policies
            .iter()
            .any(|policy| policy.authorize_request(request).is_ok())
        {
            return Err(TransportError("connector-policy"));
        }
        let access = AccessRequest::new(self.reference.clone(), "github-api", "api.github.com", 60)
            .map_err(|_| TransportError("credential"))?;
        let grant = self
            .broker
            .issue_bound(access, self.binding.clone())
            .map_err(|_| TransportError("credential"))?;
        let used = self
            .broker
            .consume_bound(grant.token(), "github-api", "api.github.com", &self.binding)
            .map_err(|_| TransportError("credential"))?;
        used.secret.expose(|token| {
            crate::github_provider_action::perform(
                request,
                token,
                &self.curl_path,
                &self.state_path,
                &self.artifacts,
            )
        })
    }
}

impl GitHubProvider for PlatformGitHubProvider {
    fn perform(
        &mut self,
        request: &GitHubApiRequest,
    ) -> Result<GitHubBackendReceipt, TransportError> {
        self.execute(request)
    }
}

fn binding(value: &GitHubAuthorizationV1) -> Result<LeaseBinding, TransportError> {
    LeaseBinding::new(
        &value.pairwise_subject,
        &value.device_ref,
        &value.workload_id,
        &value.grant_id,
        &value.permission_resource,
        &value.permission_operation,
        &value.proof_key_ref,
    )
    .map_err(|_| TransportError("binding"))
}
