use zixcel_github::{GitHubAction, GitHubApiRequest, GitHubBackendReceipt};

use crate::TransportError;
use crate::github_provider_action::ActionContext;
use crate::github_provider_action_support::{json_digest, repo_path, success_json};
use crate::github_provider_http::call;

pub(crate) fn perform(
    request: &GitHubApiRequest,
    context: &ActionContext<'_>,
) -> Result<GitHubBackendReceipt, TransportError> {
    let suffix = match request.action {
        GitHubAction::ObserveRepositoryMetadata => String::new(),
        GitHubAction::ListIssues { maximum_items } => format!("issues?per_page={maximum_items}"),
        GitHubAction::ListPullRequests { maximum_items } => {
            format!("pulls?per_page={maximum_items}")
        }
        GitHubAction::ListReleases { maximum_items } => {
            format!("releases?per_page={maximum_items}")
        }
        GitHubAction::ListWorkflowRuns { maximum_items } => {
            format!("actions/runs?per_page={maximum_items}")
        }
        _ => return Err(TransportError("provider-action")),
    };
    let response = call(
        context.curl,
        "GET",
        &repo_path(request, &suffix),
        b"",
        context.token,
        context.state,
    )?;
    let value = success_json(&response, &[200])?;
    Ok(GitHubBackendReceipt {
        provider_request_id: response.request_id,
        remote_reference: format!("github://{}/{}", request.owner, request.repository),
        remote_digest_sha256: Some(json_digest(&value)?),
    })
}
