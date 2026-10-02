# crowsi-github-transport interface reference

Use the [usage guide](getting-started.md) for the first steps. This reference preserves the current interface details and operational limits. Run command examples from the repository root, after preparing the exact declared dependencies and registered configuration.

## Acceptance

Use digest-pinned artifacts and configured trust/custody. Test stale grants,
wrong targets, duplicate operations, unknown results and independent result
verification. Provider test fixtures verify the contract, not a real connected
GitHub deployment. Declare that distinction in the consumer's readiness state.

## Configured capability ceilings

Transport configuration accepts `connector_configs`, an array of Zixcel connector configurations with schema, config_id, opaque connection_ref, organization, repositories and allowed_actions. Missing policies reject every provider call, including verification. A matching policy permits reads; writes require explicit action names. The policy is applied before custody issuance and again in execute_command, alongside existing signature, target, time and ledger checks. Never copy the Codex connector credentials into configuration.

Organization creation uses the organization endpoint. Snapshot transport uses string parent SHAs, a complete replacement tree and force=false ref updates. Artifact references use sha256-prefixed strings; digest fields use bare 64-character hexadecimal values. Delete is restricted to a private repository with the expected immutable repository ID and matching content-addressed backup declaration. GitHub enforces actual token and organization administration rights.

Local owner-crate changes must be released into the immutable private registry and adopted by a new signed package binding before production use. Registry dependencies remain in Cargo.toml; local validation supplies explicit Cargo patch settings, not a hidden cross-repository dependency. No installed generation or signed binding was changed.
