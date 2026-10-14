# Contributing

Issues and pull requests are welcome.

## Checks

Run these before CI so you don't have to do a round trip.

```sh
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --check
cargo doc --no-deps
```

CI also treats any warning from `cargo doc` as an error, runs the tests on beta
and nightly as well as stable, and checks that the crate still builds with the
Rust version in `rust-version`. That last check needs [cargo-hack].

```sh
cargo hack check --rust-version --workspace --all-targets --ignore-private
```

## Changes

Anything a user of the crate would notice gets an entry under `Unreleased` in
[CHANGELOG.md](CHANGELOG.md), and breaking changes are marked **Breaking**.

## Releasing

1. Move the entries under `Unreleased` in `CHANGELOG.md` to a new version.
2. Raise `version` in `Cargo.toml` and commit.
3. Tag the commit with an annotated tag, then push the commit and the tag.

   ```sh
   git tag -a vX.Y.Z -m vX.Y.Z
   git push origin HEAD vX.Y.Z
   ```

4. Run `cargo publish`.

[cargo-hack]: https://github.com/taiki-e/cargo-hack
