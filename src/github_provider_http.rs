use std::fmt::Write as _;
use std::fs::{OpenOptions, read, read_to_string};
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use zeroize::Zeroize;

use crate::TransportError;

pub(crate) struct HttpResponse {
    pub status: u16,
    pub request_id: String,
    pub body: Vec<u8>,
}

pub(crate) fn call(
    curl: &str,
    method: &str,
    path: &str,
    body: &[u8],
    token: &[u8],
    state_path: &Path,
) -> Result<HttpResponse, TransportError> {
    let token = std::str::from_utf8(token).map_err(|_| TransportError("credential"))?;
    if token.is_empty()
        || token.len() > 512
        || token
            .bytes()
            .any(|value| !value.is_ascii_graphic() || matches!(value, b'"' | b'\\'))
    {
        return Err(TransportError("credential"));
    }
    let files = ExchangeFiles::create(state_path, body)?;
    let mut config = config(method, path, token, &files);
    let mut child = Command::new(curl)
        .arg("--config")
        .arg("-")
        .env_clear()
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| TransportError("provider-unavailable"))?;
    child
        .stdin
        .take()
        .ok_or(TransportError("provider-unavailable"))?
        .write_all(config.as_bytes())
        .map_err(|_| TransportError("provider-unavailable"))?;
    config.zeroize();
    let output = child
        .wait_with_output()
        .map_err(|_| TransportError("provider-unavailable"))?;
    if !output.status.success() || output.stdout.len() > 3 {
        return Err(TransportError("provider-unavailable"));
    }
    let status = std::str::from_utf8(&output.stdout)
        .ok()
        .and_then(|value| value.parse().ok())
        .ok_or(TransportError("provider-response"))?;
    let response = read(&files.response).map_err(|_| TransportError("provider-response"))?;
    if response.len() > 2 * 1024 * 1024 {
        return Err(TransportError("provider-response"));
    }
    let headers =
        read_to_string(&files.headers).map_err(|_| TransportError("provider-response"))?;
    let request_id = headers
        .lines()
        .find_map(|line| {
            line.strip_prefix("x-github-request-id:")
                .or_else(|| line.strip_prefix("X-GitHub-Request-Id:"))
                .map(str::trim)
                .map(str::to_owned)
        })
        .ok_or(TransportError("provider-response"))?;
    Ok(HttpResponse {
        status,
        request_id,
        body: response,
    })
}

fn config(method: &str, path: &str, token: &str, files: &ExchangeFiles) -> String {
    let mut value = format!(
        "silent\nshow-error\nrequest = \"{method}\"\nurl = \"https://api.github.com{path}\"\n\
         header = \"Accept: application/vnd.github+json\"\n\
         header = \"Authorization: Bearer {token}\"\n\
         header = \"X-GitHub-Api-Version: 2022-11-28\"\n\
         output = \"{}\"\ndump-header = \"{}\"\nwrite-out = \"%{{http_code}}\"\n\
         max-time = 30\nproto = \"=https\"\ntlsv1.2\n",
        files.response.display(),
        files.headers.display(),
    );
    if method != "GET" {
        let _ = write!(
            value,
            "header = \"Content-Type: application/json\"\ndata-binary = \"@{}\"\n",
            files.body.display()
        );
    }
    value
}

struct ExchangeFiles {
    body: PathBuf,
    headers: PathBuf,
    response: PathBuf,
}

impl ExchangeFiles {
    fn create(state_path: &Path, body: &[u8]) -> Result<Self, TransportError> {
        let parent = state_path.parent().ok_or(TransportError("state-path"))?;
        let mut nonce = [0_u8; 16];
        getrandom::fill(&mut nonce).map_err(|_| TransportError("entropy"))?;
        let prefix = parent.join(format!(".github-http-{}", hex::encode(nonce)));
        let value = Self {
            body: prefix.with_extension("body"),
            headers: prefix.with_extension("headers"),
            response: prefix.with_extension("response"),
        };
        secure_file(&value.body, body)?;
        secure_file(&value.headers, b"")?;
        secure_file(&value.response, b"")?;
        Ok(value)
    }
}

impl Drop for ExchangeFiles {
    fn drop(&mut self) {
        for path in [&self.body, &self.headers, &self.response] {
            let _ = std::fs::remove_file(path);
        }
    }
}

fn secure_file(path: &Path, bytes: &[u8]) -> Result<(), TransportError> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .map_err(|_| TransportError("state-write"))?;
    file.write_all(bytes)
        .map_err(|_| TransportError("state-write"))
}
