# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]
### Added
- The live-list now correctly mirrors the state of _all_ stations on the bus,
  not just the ones in our own GAP range.

### Fixed
- Made `gsd-parser` parse more GSD files correctly, ignoring a few more
  constructs that it currently does not care about.


## [0.1.1] - 2023-12-28
- Fix some cargo metadata.


## [0.1.0] - 2023-12-28
Initial Release.


[Unreleased]: https://github.com/rahix/profirust/compare/v0.1.1...HEAD
[0.1.1]: https://github.com/rahix/profirust/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/rahix/profirust/releases/tag/v0.1.0
