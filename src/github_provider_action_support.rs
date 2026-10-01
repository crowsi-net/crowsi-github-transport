use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use zixcel_github::{GitHubApiRequest, GitHubBackendReceipt, parse_text_artifact};

use crate::TransportError;
use crate::github_provider_action::ActionContext;
use crate::github_provider_http::{HttpResponse, call};

pub(crate) fn require_owner(
    request: &GitHubApiRequest,
    context: &ActionContext<'_>,
) -> Result<(), TransportError> {
    let response = call(
        context.curl,
        "GET",
        "/user",
        b"",
        context.token,
        context.state,
    )?;
    let value = success_json(&response, &[200])?;
    let login = value
        .get("login")
        .and_then(Value::as_str)
        .ok_or(TransportError("provider-response"))?;
    if !login.eq_ignore_ascii_case(&request.owner) {
        return Err(TransportError("provider-owner"));
    }
    Ok(())
}

pub(crate) fn text(context: &ActionContext<'_>, reference: &str) -> Result<String, TransportError> {
    parse_text_artifact(&context.artifacts.read(reference)?)
        .map(|value| value.text)
        .map_err(|_| TransportError("artifact"))
}

pub(crate) fn post(
    request: &GitHubApiRequest,
    context: &ActionContext<'_>,
    path: &str,
    body: &Value,
    statuses: &[u16],
) -> Result<GitHubBackendReceipt, TransportError> {
    let bytes = serde_json::to_vec(body).map_err(|_| TransportError("provider-request"))?;
    let response = call(
        context.curl,
        "POST",
        path,
        &bytes,
        context.token,
        context.state,
    )?;
    receipt(
        &success_json(&response, statuses)?,
        request,
        &response.request_id,
    )
}

pub(crate) fn success_json(
    response: &HttpResponse,
    statuses: &[u16],
) -> Result<Value, TransportError> {
    if !statuses.contains(&response.status) {
        return Err(TransportError("provider-rejected"));
    }
    if response.body.is_empty() {
        return Ok(json!({}));
    }
    serde_json::from_slice(&response.body).map_err(|_| TransportError("provider-response"))
}

pub(crate) fn required<'a>(value: &'a Value, pointer: &str) -> Result<&'a str, TransportError> {
    value
        .pointer(pointer)
        .and_then(Value::as_str)
        .ok_or(TransportError("provider-response"))
}

fn receipt(
    value: &Value,
    request: &GitHubApiRequest,
    request_id: &str,
) -> Result<GitHubBackendReceipt, TransportError> {
    let reference = value
        .get("html_url")
        .or_else(|| value.get("url"))
        .and_then(Value::as_str)
        .map_or_else(
            || format!("github://{}/{}", request.owner, request.repository),
            str::to_owned,
        );
    Ok(GitHubBackendReceipt {
        provider_request_id: request_id.to_owned(),
        remote_reference: reference,
        remote_digest_sha256: Some(json_digest(value)?),
    })
}

pub(crate) fn repo_path(request: &GitHubApiRequest, suffix: &str) -> String {
    if suffix.is_empty() {
        format!("/repos/{}/{}", request.owner, request.repository)
    } else {
        format!("/repos/{}/{}/{}", request.owner, request.repository, suffix)
    }
}

pub(crate) fn json_digest(value: &Value) -> Result<String, TransportError> {
    let bytes = serde_json::to_vec(value).map_err(|_| TransportError("provider-response"))?;
    Ok(format!("sha256:{}", hex::encode(Sha256::digest(bytes))))
}

pub(crate) fn component(value: &str) -> String {
    value
        .bytes()
        .flat_map(|byte| {
            if byte.is_ascii_alphanumeric() || b"-._~".contains(&byte) {
                vec![byte as char]
            } else {
                format!("%{byte:02X}").chars().collect()
            }
        })
        .collect()
}
