use crowsi_github_transport::{
    ExecutionLedger, ExecutionStart, GitHubProvider, GitHubTransportConfigV1,
    PlatformGitHubProvider, TransportError, execute_command,
};
use crowsi_provider_egress_contracts::{
    GitHubEgressCommandV1, GitHubEgressMode, authorization_digest,
};
use std::io::Read;
use std::time::{SystemTime, UNIX_EPOCH};
use zixcel_github::{GitHubApiRequest, GitHubBackendReceipt};

fn main() {
    if let Err(error) = run() {
        eprintln!("{}", error.0);
        std::process::exit(1);
    }
}

fn run() -> Result<(), TransportError> {
    let config_path = config_path()?;
    let config_bytes = std::fs::read(config_path).map_err(|_| TransportError("config-read"))?;
    let config = GitHubTransportConfigV1::parse(&config_bytes)?;
    let mut input = Vec::new();
    std::io::stdin()
        .take(2_097_153)
        .read_to_end(&mut input)
        .map_err(|_| TransportError("request-read"))?;
    if input.len() > 2_097_152 {
        return Err(TransportError("request-size"));
    }
    let command: GitHubEgressCommandV1 =
        serde_json::from_slice(&input).map_err(|_| TransportError("request-json"))?;
    require_current_time(command.now_epoch_s)?;
    match command.mode {
        GitHubEgressMode::Verify => {
            write(&execute_command(&command, &config, &mut DeniedProvider)?)
        }
        GitHubEgressMode::Execute => execute(&command, &config),
    }
}

fn execute(
    command: &GitHubEgressCommandV1,
    config: &GitHubTransportConfigV1,
) -> Result<(), TransportError> {
    let mut verify = command.clone();
    verify.mode = GitHubEgressMode::Verify;
    execute_command(&verify, config, &mut DeniedProvider)?;
    let digest = authorization_digest(&command.authorization).map_err(TransportError)?;
    let ledger = ExecutionLedger::new(&config.state_path);
    match ledger.begin(&command.authorization.authorization_id, &digest)? {
        ExecutionStart::Completed(receipt) => {
            write(&serde_json::to_value(receipt).map_err(|_| TransportError("receipt"))?)
        }
        ExecutionStart::Execute => {
            let mut provider = PlatformGitHubProvider::new(config, command)?;
            let value = execute_command(command, config, &mut provider)?;
            let receipt =
                serde_json::from_value(value.clone()).map_err(|_| TransportError("receipt"))?;
            ledger.complete(&command.authorization.authorization_id, &digest, receipt)?;
            write(&value)
        }
    }
}

fn config_path() -> Result<String, TransportError> {
    let mut args = std::env::args().skip(1);
    match (args.next().as_deref(), args.next(), args.next()) {
        (Some("--config"), Some(path), None) if std::path::Path::new(&path).is_absolute() => {
            Ok(path)
        }
        _ => Err(TransportError("usage")),
    }
}

fn require_current_time(value: u64) -> Result<(), TransportError> {
    let current = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| TransportError("clock"))?
        .as_secs();
    if value.abs_diff(current) > 5 {
        return Err(TransportError("clock"));
    }
    Ok(())
}

fn write(value: &serde_json::Value) -> Result<(), TransportError> {
    serde_json::to_writer(std::io::stdout(), value).map_err(|_| TransportError("response-write"))
}

struct DeniedProvider;
impl GitHubProvider for DeniedProvider {
    fn perform(&mut self, _: &GitHubApiRequest) -> Result<GitHubBackendReceipt, TransportError> {
        Err(TransportError("verify-executed"))
    }
}
