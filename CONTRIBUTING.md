# Contributing

Thanks for your interest in ebs-wees-bill.

## Building and testing

```sh
cargo build
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

These checks must pass before a change is merged, on stable and on Rust 1.85. A change that alters behaviour comes with a test that fails without it, and with an update of `README.md`, `README.nl.md`, `QUALITY.md`, and the `[Unreleased]` part of `CHANGELOG.md`. A count in the documents (tests, claims) is a measurement made after the last change, not an estimate.

Test fixtures must be synthetic, or anonymised copies of a public post. Never add a real access key, a real account number, or a real AWS export.

## Commit messages

Commit messages are short English sentences that say what changed, for example `Escape controls in values shown on the report`.

## Contact

PEPAI, pep_ai@icloud.com
