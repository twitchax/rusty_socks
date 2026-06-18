# Development

## Common tasks

```bash
cargo make ci        # format check + clippy (-D warnings) + tests
cargo make test      # cargo nextest run
cargo make cov       # tests with coverage
cargo make release   # run all checks, then publish to crates.io (needs `cargo login` first)
```

## Releasing

1. Bump `version` in `Cargo.toml`.
2. `cargo make release` — checks, then publishes the crate to crates.io.
3. Push a `vX.Y.Z` tag — CI builds the per-OS binaries and attaches them to the GitHub Release.

## TODO

- IPv6 listen / endpoint interface support.
- Bounds-check `Request::from_data` / `Handshake::from_data` against malformed or short input (they currently index directly, which panics and aborts that single connection task).
- More connection-lifecycle tests (idle reaping over real sockets, CIDR rejection paths).
