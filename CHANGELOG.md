# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]
### `profirust`
#### Fixed
- Fixed compiler warnings due to superfluous `#[cfg]` gates.

### `gsd-parser`
#### Fixed
- Fixed a lot of panics caused by invalid input.  Instead, gsd-parser now
  propagates an error for the caller to handle.


## [0.2.1] - 2024-05-09
### `gsdtool`
#### Fixed
- Fixed `gsdtool` not using the correct dependency version of `gsd-parser`.


## [0.2.0] - 2024-05-09
### `profirust`
#### Added
- The live-list now correctly mirrors the state of _all_ stations on the bus,
  not just the ones in our own GAP range.
#### Fixed
- Improve robustness of the FDL layer against unexpected situations.

### `gsd-parser`/`gsdtool`
#### Added
- Added support in `gsd-parser` for the older
  `User_Prm_Data`/`User_Prm_Data_Len` fields when no `Ext_User_Prm_*` data is
  present.
- `gsd-parser` now supports the `Changeable` and `Visible` settings for
  ExtUserPrmData.  `gsdtool` also honors these settings now.
- Added more prompting variants to `gsdtool` to prompt for even more possible
  settings.

#### Fixed
- Made `gsd-parser` parse more GSD files correctly, ignoring a few more
  constructs that it currently does not care about.
- Fixed `gsd-parser` not being case-insensitive for some keywords.
- Fixed `gsd-parser` not parsing negative numbers correctly.


## [0.1.1] - 2023-12-28
- Fix some cargo metadata.


## [0.1.0] - 2023-12-28
Initial Release.


[Unreleased]: https://github.com/rahix/profirust/compare/v0.2.1...HEAD
[0.2.1]: https://github.com/rahix/profirust/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/rahix/profirust/compare/v0.1.1...v0.2.0
[0.1.1]: https://github.com/rahix/profirust/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/rahix/profirust/releases/tag/v0.1.0
