## `gsd-parser` regression testsuite
You can place arbitrary `*.gsd` files in this directory to run `gsd-parser` regression tests against them.

**Careful**: You will need to `cargo clean` before the testsuite picks up new files.

The regression tests are implemented using [insta](https://insta.rs/) &mdash; check its documentation for details on usage.
