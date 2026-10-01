use crowsi_provider_egress_contracts::GitHubEgressReceiptV1;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::TransportError;

const SCHEMA: &str = "crowsi://github-transport/execution-ledger/v1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecutionStart {
    Execute,
    Completed(GitHubEgressReceiptV1),
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Document {
    schema: String,
    records: BTreeMap<String, Record>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "state", rename_all = "kebab-case", deny_unknown_fields)]
enum Record {
    Prepared {
        request_digest_sha256: String,
    },
    Completed {
        request_digest_sha256: String,
        receipt: GitHubEgressReceiptV1,
    },
}

pub struct ExecutionLedger {
    path: PathBuf,
}

impl ExecutionLedger {
    #[must_use]
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    /// Reserves one authorization before provider execution.
    ///
    /// # Errors
    ///
    /// Rejects replay substitution, an ambiguous prior attempt, or persistence failure.
    pub fn begin(&self, id: &str, digest: &str) -> Result<ExecutionStart, TransportError> {
        let mut document = load(&self.path)?;
        match document.records.get(id) {
            Some(Record::Completed {
                request_digest_sha256,
                receipt,
            }) if request_digest_sha256 == digest => {
                return Ok(ExecutionStart::Completed(receipt.clone()));
            }
            Some(Record::Prepared {
                request_digest_sha256,
            }) if request_digest_sha256 == digest => {
                return Err(TransportError("provider-outcome-unknown"));
            }
            Some(_) => return Err(TransportError("authorization-replay")),
            None => {}
        }
        if document.records.len() >= 1_024 {
            return Err(TransportError("ledger-capacity"));
        }
        document.records.insert(
            id.to_owned(),
            Record::Prepared {
                request_digest_sha256: digest.to_owned(),
            },
        );
        store(&self.path, &document)?;
        Ok(ExecutionStart::Execute)
    }

    /// Commits a metadata-only provider receipt after execution.
    ///
    /// # Errors
    ///
    /// Rejects a missing or mismatched reservation and persistence failure.
    pub fn complete(
        &self,
        id: &str,
        digest: &str,
        receipt: GitHubEgressReceiptV1,
    ) -> Result<(), TransportError> {
        let mut document = load(&self.path)?;
        match document.records.get(id) {
            Some(Record::Prepared {
                request_digest_sha256,
            }) if request_digest_sha256 == digest => {}
            _ => return Err(TransportError("ledger-correlation")),
        }
        document.records.insert(
            id.to_owned(),
            Record::Completed {
                request_digest_sha256: digest.to_owned(),
                receipt,
            },
        );
        store(&self.path, &document)
    }
}

fn load(path: &Path) -> Result<Document, TransportError> {
    if !path.exists() {
        return Ok(Document {
            schema: SCHEMA.into(),
            records: BTreeMap::new(),
        });
    }
    let bytes = std::fs::read(path).map_err(|_| TransportError("ledger-read"))?;
    if bytes.len() > 4 * 1024 * 1024 {
        return Err(TransportError("ledger-size"));
    }
    let value: Document = serde_json::from_slice(&bytes).map_err(|_| TransportError("ledger"))?;
    if value.schema != SCHEMA || value.records.len() > 1_024 {
        return Err(TransportError("ledger"));
    }
    Ok(value)
}

fn store(path: &Path, value: &Document) -> Result<(), TransportError> {
    let bytes = serde_json::to_vec(value).map_err(|_| TransportError("ledger"))?;
    if bytes.len() > 4 * 1024 * 1024 {
        return Err(TransportError("ledger-size"));
    }
    crate::ledger_io::replace(path, &bytes)
}
