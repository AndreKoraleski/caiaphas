# caiaphas

[![CI][ci-badge]][ci]
[![crates.io][crates-badge]][crates]
[![docs.rs][docs-badge]][docs]

EBNF parser generator for Rust that detects grammar ambiguities and suggests
fixes

## Installation

```sh
cargo add caiaphas
```

## Usage

```rust
use caiaphas::add;

assert_eq!(add(2, 2), 4);
```

## Minimum supported Rust version

Rust 1.85 or later.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md).

## License

Licensed under either of

* Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or
  http://www.apache.org/licenses/LICENSE-2.0)
* MIT license ([LICENSE-MIT](LICENSE-MIT) or
  http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall
be dual licensed as above, without any additional terms or conditions.

[ci-badge]: https://github.com/AndreKoraleski/caiaphas/actions/workflows/ci.yml/badge.svg
[ci]: https://github.com/AndreKoraleski/caiaphas/actions/workflows/ci.yml
[crates-badge]: https://img.shields.io/crates/v/caiaphas.svg
[crates]: https://crates.io/crates/caiaphas
[docs-badge]: https://img.shields.io/docsrs/caiaphas
[docs]: https://docs.rs/caiaphas
