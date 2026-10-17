# caiaphas-cli

The `caiaphas` command checks ISO/IEC 14977 EBNF grammars as the `grammar!`
macro of [caiaphas] does, without building anything. It prints every finding,
suggestions included, and explains each one by its code, with an example and
the usual fix. `caiaphas explain` without a code lists every code.
`caiaphas types` prints the types the macro would generate, each after its
rule.

```sh
cargo install caiaphas-cli
caiaphas check grammars/json.ebnf
caiaphas types grammars/json.ebnf
caiaphas explain tied-branches
```

It exits with 1 when a grammar cannot be read, has a problem that would stop
the build, or a code names no finding, so `caiaphas check` suits a CI step.

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

[caiaphas]: https://crates.io/crates/caiaphas
