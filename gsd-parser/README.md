`gsd-parser` [![crates.io page](https://img.shields.io/crates/v/gsd-parser.svg)](https://crates.io/crates/gsd-parser) [![docs.rs page](https://docs.rs/gsd-parser/badge.svg)](https://docs.rs/gsd-parser)
============
A parser for PROFIBUS GSD (Generic Station Description) files.

## Testsuite
`gsd-parser` comes with a testsuite to catch parsing regressions.  The
testsuite uses [insta.rs](https://insta.rs/).  To use it, populate
`tests/data/` with all `.gsd` files you have.  Then run `cargo insta test` to
generate first snapshots.  You'll need to accept each snapshot using `cargo
insta review`.  From then on, you can just run `cargo test` as usual.

## License
Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](../LICENSE-APACHE) or
  <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](../LICENSE-MIT) or
  <http://opensource.org/licenses/MIT>)

at your option.

### Contribution
Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall
be dual licensed as above, without any additional terms or conditions.
