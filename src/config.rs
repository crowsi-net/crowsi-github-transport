use crowsi_credential_broker::PlatformCustodyConfigV1;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::TransportError;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GitHubTransportConfigV1 {
    pub schema: String,
    pub trust: GitHubTransportTrustV1,
    pub custody: PlatformCustodyConfigV1,
    pub connections: BTreeMap<String, String>,
    #[serde(default)]
    pub connector_configs: Vec<zixcel_github::ConnectorConfig>,
    pub artifact_root: String,
    pub state_path: String,
    pub curl_path: String,
    pub curl_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GitHubTransportTrustV1 {
    pub issuer: String,
    pub audience: String,
    pub key_id: String,
    pub public_key_hex: String,
    pub minimum_issued_at_epoch_s: u64,
}

impl GitHubTransportConfigV1 {
    /// Parses one closed public trust document.
    ///
    /// # Errors
    ///
    /// Rejects oversized, malformed, unknown, or invalid trust fields.
    pub fn parse(bytes: &[u8]) -> Result<Self, TransportError> {
        if bytes.is_empty() || bytes.len() > 16_384 {
            return Err(TransportError("config-size"));
        }
        let value: Self =
            serde_json::from_slice(bytes).map_err(|_| TransportError("config-json"))?;
        let trust = &value.trust;
        if value.schema != "crowsi://github-transport/config/v1"
            || !reference(&trust.issuer, 256)
            || trust.audience != "crowsi-github-transport"
            || !identifier(&trust.key_id, 128)
            || trust.public_key_hex.len() != 64
            || !trust.public_key_hex.bytes().all(lower_hex)
            || value.connections.is_empty()
            || value.connections.len() > 64
            || value
                .connections
                .iter()
                .any(|(key, item)| !sha256(key) || !reference(item, 512))
            || !absolute_path(&value.artifact_root)
            || !absolute_path(&value.state_path)
            || !absolute_path(&value.curl_path)
            || !sha256(&value.curl_sha256)
        {
            return Err(TransportError("config-trust"));
        }
        if value.connector_configs.len() > 64
            || value
                .connector_configs
                .iter()
                .any(|policy| policy.validate().is_err())
        {
            return Err(TransportError("connector-policy"));
        }
        crate::file_identity::verify_executable(&value.curl_path, &value.curl_sha256)?;
        Ok(value)
    }
}

fn lower_hex(value: u8) -> bool {
    value.is_ascii_digit() || (b'a'..=b'f').contains(&value)
}
fn reference(value: &str, maximum: usize) -> bool {
    !value.is_empty()
        && value.len() <= maximum
        && value.trim() == value
        && !value.bytes().any(|byte| byte.is_ascii_control())
}
fn identifier(value: &str, maximum: usize) -> bool {
    reference(value, maximum)
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"-._:/".contains(&byte))
}
fn sha256(value: &str) -> bool {
    value.len() == 71 && value.starts_with("sha256:") && value[7..].bytes().all(lower_hex)
}
fn absolute_path(value: &str) -> bool {
    reference(value, 4096) && std::path::Path::new(value).is_absolute()
}
