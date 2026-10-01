use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use zixcel_github::{
    GitHubApiRequest, GitHubBackendReceipt, GitHubRemoteExpectation, parse_snapshot_artifact,
};

use crate::TransportError;
use crate::github_provider_action::ActionContext;
use crate::github_provider_action_support::{
    component, json_digest, repo_path, required, success_json,
};
use crate::github_provider_http::call;

pub(crate) fn push(
    request: &GitHubApiRequest,
    context: &ActionContext<'_>,
    reference: &str,
    digest: &str,
    expected: &GitHubRemoteExpectation,
) -> Result<GitHubBackendReceipt, TransportError> {
    if reference != format!("sha256:{digest}") {
        return Err(TransportError("snapshot-binding"));
    }
    let snapshot = parse_snapshot_artifact(&context.artifacts.read(reference)?)
        .map_err(|_| TransportError("snapshot"))?;
    let parent = observed_parent(request, context, &snapshot.branch, expected)?;
    let mut entries = Vec::with_capacity(snapshot.files.len());
    for file in &snapshot.files {
        let value = post(
            request,
            context,
            "git/blobs",
            &json!({
                "content": file.content_base64, "encoding": "base64",
            }),
            &[201],
        )?;
        entries.push(
            json!({"path": file.path, "mode": file.mode.as_str(), "type": "blob",
            "sha": required(&value, "/sha")?}),
        );
    }
    // A complete source snapshot replaces the tree; the parent commit retains history.
    let tree_body = json!({"tree": entries});
    let tree = post(request, context, "git/trees", &tree_body, &[201])?;
    let tree_sha = required(&tree, "/sha")?;
    let parents = parent
        .as_ref()
        .map(|sha| vec![json!(sha)])
        .unwrap_or_default();
    let commit = post(
        request,
        context,
        "git/commits",
        &json!({
            "message": snapshot.message, "tree": tree_sha, "parents": parents,
        }),
        &[201],
    )?;
    let commit_sha = required(&commit, "/sha")?.to_owned();
    let (method, path, body, statuses) = if parent.is_some() {
        (
            "PATCH",
            repo_path(
                request,
                &format!("git/refs/heads/{}", component(&snapshot.branch)),
            ),
            json!({"sha": commit_sha.clone(), "force": false}),
            vec![200],
        )
    } else {
        (
            "POST",
            repo_path(request, "git/refs"),
            json!({"ref": format!("refs/heads/{}", snapshot.branch), "sha": commit_sha.clone()}),
            vec![201],
        )
    };
    let bytes = serde_json::to_vec(&body).map_err(|_| TransportError("provider-request"))?;
    let response = call(
        context.curl,
        method,
        &path,
        &bytes,
        context.token,
        context.state,
    )?;
    let request_id = response.request_id.clone();
    let value = success_json(&response, &statuses)?;
    Ok(GitHubBackendReceipt {
        provider_request_id: request_id,
        remote_reference: format!(
            "github://{}/{}/commit/{commit_sha}",
            request.owner, request.repository
        ),
        remote_digest_sha256: Some(json_digest(&value)?),
    })
}

fn post(
    request: &GitHubApiRequest,
    context: &ActionContext<'_>,
    suffix: &str,
    body: &Value,
    statuses: &[u16],
) -> Result<Value, TransportError> {
    let body = serde_json::to_vec(&body).map_err(|_| TransportError("provider-request"))?;
    let response = call(
        context.curl,
        "POST",
        &repo_path(request, suffix),
        &body,
        context.token,
        context.state,
    )?;
    success_json(&response, statuses)
}

fn observed_parent(
    request: &GitHubApiRequest,
    context: &ActionContext<'_>,
    branch: &str,
    expected: &GitHubRemoteExpectation,
) -> Result<Option<String>, TransportError> {
    let reference_path = repo_path(request, &format!("git/ref/heads/{}", component(branch)));
    let current = call(
        context.curl,
        "GET",
        &reference_path,
        b"",
        context.token,
        context.state,
    )?;
    match expected {
        GitHubRemoteExpectation::Absent if current.status == 404 => Ok(None),
        GitHubRemoteExpectation::Exact { commit_sha256 } if current.status == 200 => {
            let value = success_json(&current, &[200])?;
            let sha = value
                .pointer("/object/sha")
                .and_then(Value::as_str)
                .ok_or(TransportError("provider-response"))?;
            let actual = hex::encode(Sha256::digest(sha.as_bytes()));
            if &actual != commit_sha256 {
                return Err(TransportError("remote-expectation"));
            }
            Ok(Some(sha.to_owned()))
        }
        _ => Err(TransportError("remote-expectation")),
    }
}
