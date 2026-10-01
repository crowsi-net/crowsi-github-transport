#![forbid(unsafe_code)]
#![doc = "Credential-bound, authorization-verifying GitHub transport for Crowsi."]

mod artifact_store;
#[cfg(test)]
mod artifact_store_tests;
mod authorization;
mod config;
mod execute;
mod file_identity;
mod github_provider;
mod github_provider_action;
mod github_provider_action_read;
mod github_provider_action_support;
mod github_provider_action_write;
mod github_provider_http;
mod github_provider_snapshot;
mod ledger;
mod ledger_io;
mod model;

pub use config::{GitHubTransportConfigV1, GitHubTransportTrustV1};
pub use execute::{GitHubProvider, execute_command};
pub use github_provider::PlatformGitHubProvider;
pub use ledger::{ExecutionLedger, ExecutionStart};
pub use model::TransportError;

#[cfg(test)]
mod provider_tests;
