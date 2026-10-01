use std::path::Path;
use zixcel_github::{GitHubApiRequest, GitHubBackendReceipt};

use crate::TransportError;
use crate::artifact_store::ArtifactStore;

pub(crate) struct ActionContext<'a> {
    pub token: &'a [u8],
    pub curl: &'a str,
    pub state: &'a Path,
    pub artifacts: &'a ArtifactStore,
}

pub(crate) fn perform(
    request: &GitHubApiRequest,
    token: &[u8],
    curl: &str,
    state: &Path,
    artifacts: &ArtifactStore,
) -> Result<GitHubBackendReceipt, TransportError> {
    let context = ActionContext {
        token,
        curl,
        state,
        artifacts,
    };
    match crate::github_provider_action_write::perform(request, &context) {
        Err(TransportError("not-write-action")) => {
            crate::github_provider_action_read::perform(request, &context)
        }
        result => result,
    }
}
