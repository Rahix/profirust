# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]
### `gsd-parser`
#### Changed
- Make `PrmBuilder::set_prm()` and `PrmBuilder::set_prm_from_text()` fallible
  instead of panicking when invalid values/prm information is provided.
- General code improvements in a few places.

### `profirust`
#### Changed
- General code improvements in a few places.


## [0.6.0] - 2025-02-23
### `profirust`
#### Changed
- Upgraded to `rp2040-hal` version 0.10.

### `gsd-parser`
#### Added
- Slot information is now parsed.
- Module `Info_Text` is now parsed.

#### Fixed
- The `max_modules` field is now forced to the correct value `1` for compact
  stations in all situations.
- Long lines (using `\`) are now parsed correctly, in whitespace and in strings.
- gsd-parser now correctly ignores any text before the `#Profibus_DP` marker.
  Some GSD files store additional settings above this marker which previously
  triggered parsing errors.

### `gsdtool`
#### Added
- `gsdtool` now uses slot information to preselect modules and filter the
  module list for each slot so only allowed modules can be selected.  This
  should make it easier to generate correct configurations.
- `gsdtool` now automatically selects the module for compact stations.


## [0.5.1] - 2025-01-17
### `profirust`
#### Added
- `SerialPortPhy` now automatically configures low-latency mode for USB-serial
  adapters on Linux.  This is especially necessary for FTDI-based devices as
  those come with a high latency configured by default.

#### Fixed
- Fixed a regression from `v0.5.0` where `no_std` builds of profirust were no
  longer possible.  They would fail with errors like the following one:
  ```text
  error[E0277]: the trait bound `ManagedSlice<'_, u8>: From<[_; 0]>` is not satisfied
  ```


## [0.5.0] - 2024-12-20
### `profirust`
#### Added
- Added a `SerialPortPhy` implementation (feature `phy-serial`) which uses an
  arbitrary OS serial device.  This is a portable PHY for using USB-RS485
  converters on any platform.
- Added a `debug-measure-roundtrip` feature to enable debug-logging for DP
  data-exchange communication roundtrip times.  This is useful for finding out
  the communication delays in a hardware setup.
- Added a `debug-measure-dp-cycle` feature to enable debug-logging of the DP
  cycle times.  That is the time between each report of `cycle_completed`, so
  one data-exchange with each peripheral attached to this DP master.

#### Changed
- **BREAKING** Buffers in the `dp::Peripheral` are now stored in `managed::ManagedSlice`
  containers.  This allows using owned buffers on platforms with an allocator.
  You may need to explicitly cast buffers into slices now, to avoid a compiler error:
  ```diff
   dp::Peripheral::new(
       ENCODER_ADDRESS,
       options,
  -    &mut buffer_inputs,
  -    &mut buffer_outputs,
  +    &mut buffer_inputs[..],
  +    &mut buffer_outputs[..],
   )
  -.with_diag_buffer(&mut buffer_diagnostics),
  +.with_diag_buffer(&mut buffer_diagnostics[..])
  ```
- **BREAKING** The `phy-linux` feature is no longer enabled by default.
- Dropped unnecessary lifetime from the `LinuxRs485Phy`.
- Changed all examples to use the new `SerialPortPhy` so they are now all
  platform-independent!

#### Fixed
- Fixed a crash of the active station when receiving a token telegram with
  invalid addresses.
- Fixed not tracking partially-received telegrams correctly.  This would show
  up as _"# bytes in the receive buffer and we go into transmission?"_ warnings
  in some setups.
- Fixed profirust not immediately processing all received telegrams at once.
  This lead to very slow performance in some multi-master environments.

### `gsd-parser`
#### Fixed
- Fixed consecutive newlines in some places tripping up the parser.

### `gsdtool`
#### Fixed
- Fixed gsdtool failing to generate an appropriate configuration for DP compact
  stations (compact stations are non-modular stations).


## [0.4.0] - 2024-11-15
### `profirust`
#### Added
- Reimplemented the FDL layer for correct multi-master operation.
- Added more checks to the Linux PHY implementation to catch serial devices
  that did not accept the required configuration.
- Added a "live-list" application (`fdl::live_list::LiveList`) which replaces
  the old built-in live-list.
- Added a "DP scanner" application (`dp::scan::DpScanner`) which scans the bus
  for any DP peripherals.
- Added support for running multiple applications ontop of a single FDL active
  station.

#### Changed
- **BREAKING** The FDL layer driver is now called `FdlActiveStation` instead of `FdlMaster`.
- **BREAKING** In the DP diagnostics, the `master_address` is now of type
  `Option<Address>`.  It is `None` when a peripheral is not yet tied to a
  specific master (previously, 255 was returned).
- **BREAKING** The `fdl.poll()` no longer returns the application events.
  These are now accessed via a specific method on the application types, e.g.
  `DpMaster::take_last_events()`.  In code, this requires a change like this:
  ```diff
  -        let events = fdl.poll(now, &mut phy, &mut dp_master);
  +        fdl.poll(now, &mut phy, &mut dp_master);
  +        let events = dp_master.take_last_events();
  ```

#### Removed
- **BREAKING** Removed the live-list that was built into the FDL layer driver.


## [0.3.0] - 2024-10-31
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


[Unreleased]: https://github.com/rahix/profirust/compare/v0.6.0...HEAD
[0.6.0]: https://github.com/rahix/profirust/compare/v0.5.1...v0.6.0
[0.5.1]: https://github.com/rahix/profirust/compare/v0.5.0...v0.5.1
[0.5.0]: https://github.com/rahix/profirust/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/rahix/profirust/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/rahix/profirust/compare/v0.2.1...v0.3.0
[0.2.1]: https://github.com/rahix/profirust/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/rahix/profirust/compare/v0.1.1...v0.2.0
[0.1.1]: https://github.com/rahix/profirust/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/rahix/profirust/releases/tag/v0.1.0
