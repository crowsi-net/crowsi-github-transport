use serde_json::json;
use zixcel_github::{
    GitHubAction, GitHubApiRequest, GitHubBackendReceipt, parse_workflow_inputs_artifact,
};

use crate::TransportError;
use crate::github_provider_action::ActionContext;
use crate::github_provider_action_support::{component, post, repo_path, text};

pub(crate) fn perform(
    request: &GitHubApiRequest,
    context: &ActionContext<'_>,
) -> Result<GitHubBackendReceipt, TransportError> {
    match &request.action {
        GitHubAction::CreatePrivateRepository { default_branch } => {
            create_repository(request, context, default_branch)
        }
        GitHubAction::DeleteRepository {
            expected_repository_id,
            backup_digest_sha256,
        } => delete_repository(
            request,
            context,
            expected_repository_id,
            backup_digest_sha256,
        ),
        GitHubAction::CreateIssue {
            title,
            body_artifact_ref,
        } => post(
            request,
            context,
            &repo_path(request, "issues"),
            &json!({"title": title, "body": text(context, body_artifact_ref)?}),
            &[201],
        ),
        GitHubAction::CreatePullRequest {
            base,
            head,
            title,
            body_artifact_ref,
        } => post(
            request,
            context,
            &repo_path(request, "pulls"),
            &json!({"base": base, "head": head, "title": title,
                "body": text(context, body_artifact_ref)?}),
            &[201],
        ),
        GitHubAction::DispatchWorkflow {
            workflow_ref,
            git_ref,
            inputs_artifact_ref,
        } => dispatch(
            request,
            context,
            workflow_ref,
            git_ref,
            inputs_artifact_ref.as_deref(),
        ),
        GitHubAction::PushRepositorySnapshot {
            snapshot_ref,
            snapshot_digest_sha256,
            expected_remote,
        } => crate::github_provider_snapshot::push(
            request,
            context,
            snapshot_ref,
            snapshot_digest_sha256,
            expected_remote,
        ),
        _ => Err(TransportError("not-write-action")),
    }
}

fn dispatch(
    request: &GitHubApiRequest,
    context: &ActionContext<'_>,
    workflow_ref: &str,
    git_ref: &str,
    inputs_ref: Option<&str>,
) -> Result<GitHubBackendReceipt, TransportError> {
    let inputs = inputs_ref
        .map(|reference| {
            parse_workflow_inputs_artifact(&context.artifacts.read(reference)?)
                .map(|value| value.inputs)
                .map_err(|_| TransportError("artifact"))
        })
        .transpose()?
        .unwrap_or_default();
    let path = repo_path(
        request,
        &format!("actions/workflows/{}/dispatches", component(workflow_ref)),
    );
    post(
        request,
        context,
        &path,
        &json!({"ref": git_ref, "inputs": inputs}),
        &[204],
    )
}

fn create_repository(
    request: &GitHubApiRequest,
    context: &ActionContext<'_>,
    branch: &str,
) -> Result<GitHubBackendReceipt, TransportError> {
    use crate::github_provider_action_support::success_json;
    use crate::github_provider_http::call;
    let response = call(
        context.curl,
        "GET",
        &format!("/users/{}", request.owner),
        b"",
        context.token,
        context.state,
    )?;
    let owner = success_json(&response, &[200])?;
    let login = owner["login"]
        .as_str()
        .ok_or(TransportError("provider-owner"))?;
    if !login.eq_ignore_ascii_case(&request.owner) {
        return Err(TransportError("provider-owner"));
    }
    let path = match owner["type"].as_str() {
        Some("Organization") => format!("/orgs/{}/repos", request.owner),
        Some("User") => {
            crate::github_provider_action_support::require_owner(request, context)?;
            "/user/repos".to_owned()
        }
        _ => return Err(TransportError("provider-owner")),
    };
    post(
        request,
        context,
        &path,
        &json!({"name": request.repository, "private": true,
        "auto_init": true, "default_branch": branch}),
        &[201],
    )
}

fn delete_repository(
    request: &GitHubApiRequest,
    context: &ActionContext<'_>,
    expected_id: &str,
    backup_digest: &str,
) -> Result<GitHubBackendReceipt, TransportError> {
    use crate::github_provider_action_support::{json_digest, success_json};
    use crate::github_provider_http::call;
    // Backup declaration is content-addressed and target-bound. The operator must retain the actual full-history backup.
    let backup: serde_json::Value =
        serde_json::from_slice(&context.artifacts.read(&format!("sha256:{backup_digest}"))?)
            .map_err(|_| TransportError("deletion-backup"))?;
    let keys = backup
        .as_object()
        .ok_or(TransportError("deletion-backup"))?;
    let fields = [
        "schema",
        "repository_id",
        "owner",
        "repository",
        "bundle_digest_sha256",
    ];
    if keys.len() != fields.len() || fields.iter().any(|field| !keys.contains_key(*field)) {
        return Err(TransportError("deletion-backup"));
    }
    if backup["schema"] != "zixcel://github/deletion-backup/v1"
        || backup["repository_id"] != expected_id
        || backup["owner"] != request.owner
        || backup["repository"] != request.repository
    {
        return Err(TransportError("deletion-backup"));
    }
    let bundle_digest = backup["bundle_digest_sha256"]
        .as_str()
        .ok_or(TransportError("deletion-backup"))?;
    if !bundle_digest.starts_with("sha256:")
        || bundle_digest.len() != 71
        || !bundle_digest[7..]
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(TransportError("deletion-backup"));
    }
    let path = repo_path(request, "");
    let response = call(
        context.curl,
        "GET",
        &path,
        b"",
        context.token,
        context.state,
    )?;
    let repository = success_json(&response, &[200])?;
    if repository["id"]
        .as_u64()
        .map(|id| id.to_string())
        .as_deref()
        != Some(expected_id)
        || repository["private"] != true
        || repository["full_name"].as_str().is_none_or(|name| {
            !name.eq_ignore_ascii_case(&format!("{}/{}", request.owner, request.repository))
        })
    {
        return Err(TransportError("deletion-target"));
    }
    let deleted = call(
        context.curl,
        "DELETE",
        &path,
        b"",
        context.token,
        context.state,
    )?;
    success_json(&deleted, &[204])?;
    Ok(GitHubBackendReceipt {
        provider_request_id: deleted.request_id,
        remote_reference: format!("github://{}/{}", request.owner, request.repository),
        remote_digest_sha256: Some(json_digest(
            &json!({"deleted_repository_id": expected_id, "backup_digest_sha256": backup_digest}),
        )?),
    })
}
