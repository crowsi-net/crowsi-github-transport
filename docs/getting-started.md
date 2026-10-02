# Using crowsi-github-transport

Execute typed GitHub requests while keeping credentials inside the configured custody boundary.

## Before you start

The calling application supplies the operation grant and custody configuration. This package is not a general token-export interface.

## First steps

Run from the repository root:

```sh
cargo test --locked
```

## How to assess the result

- Validate the admitted GitHub operation.
- Return bounded provider results through the declared transport.

A passing source-level check establishes only what that check observes. Keep missing configuration, unavailable services and unverified deployment paths visible.

## Continue reading

[Repository overview](../README.md)
