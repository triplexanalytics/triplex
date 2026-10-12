# Contributing

Thank you for your interest in Triplex.

## Issues

Bug reports and feature requests are welcome as GitHub issues. For a bug, include the command or
the steps, what you expected, what happened, and the version (`triplex --version`).

Use synthetic data only. Never attach real personal data, a mapping file or credentials to an
issue. If the problem only shows on real data, describe its shape (column types, row count,
unusual values) or reproduce it with generated data.

Security problems go through the private channel in [SECURITY.md](SECURITY.md), not a public
issue.

## Pull requests

Triplex is developed in a separate repository and published here, so a pull request cannot be
merged directly. A good pull request is still useful: we review it, apply the change upstream
and credit you in the commit. For anything larger than a small fix, open an issue first so we
can agree on the approach.

Before you send one:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
make -C apps/triplex-web test
```

A change that affects a privacy property needs a test that demonstrates it.

## License

By contributing you agree that your contribution is licensed under the
[Apache License 2.0](LICENSE).
