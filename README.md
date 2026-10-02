# crowsi-github-transport

Execute typed GitHub requests while keeping credentials inside the configured custody boundary.

## What you can do

- Validate the admitted GitHub operation.
- Return bounded provider results through the declared transport.

## Current scope

The calling application supplies the operation grant and custody configuration. This package is not a general token-export interface.

Package distribution is not activated by this documentation. Use the checked-in source and the declared dependency versions; published availability must be verified separately.

## Getting started

Install Rust 1.97 or newer and make the declared dependencies available. Use the configured private registry when a dependency is not distributed publicly. Run from this repository:

```sh
cargo test --locked
```

## Documentation and source

[Interface reference](docs/interface-reference.md)

[Usage guide](docs/getting-started.md)

[Examples](examples) · [Implementation and public interfaces](src) · [Verification cases](tests) · [Contributing](CONTRIBUTING.md) · [Security reporting](SECURITY.md) · [License](LICENSE) · [Attribution notices](NOTICE)
