<p align="center">
  <img src="img/logo-header.svg" alt="profirust"><br/>
  <a href="https://crates.io/crates/profirust"><img src="https://img.shields.io/crates/v/profirust.svg" alt="crates.io page" /></a>
  <a href="https://docs.rs/profirust/latest/profirust/"><img src="https://docs.rs/profirust/badge.svg" alt="docs.rs page" /></a>
  <br/>
  A PROFIBUS-DP compatible communication stack written in Rust.
</p>

## What's this?
**profirust** is a pure-Rust [PROFIBUS-DP] compatible communication stack.
PROFIBUS is an industrial bus protocol used to communicate with field devices
like remote I/O, transducers, valves, drives, etc.

If you want to learn more, I suggest reading my blog posts about
[profirust][blog-post] or my [PROFIBUS Primer][blog-profibus].

[blog-post]: https://blog.rahix.de/profirust/
[blog-profibus]: https://blog.rahix.de/profibus-primer/

## Project Status
**profirust** works well for the features it currently supports, however it has
not proven itself in a real application yet.  There are still some features
missing which are needed for production use.  Check the roadmap below.

At this time, **profirust** is developed as a spare time project.  If you are
interested in this project, help is gladly accepted in the following forms:

- Code Contributions
- Donation of PROFIBUS peripherals or other equipment for testing purposes
- Funding of access to the needed IEC standards for improving compliance
- Reporting any kinds of issues encountered while using **profirust**

## Roadmap
- [x] Single-master bus up to 6 Mbit/s
- [x] Cyclic communication with DP-V0 peripherals
- [x] Basic Diagnostics
- [x] Extended Diagnostics (DP-V0)
- [ ] Multi-master bus
- [ ] Bus error tracking
- [ ] Bus discovery utilities
- [ ] Reliable communication at 12 Mbit/s
- [ ] Communication with DP-V1 peripherals
- [ ] Equidistant bus cycle
- [ ] Isochronous bus cycle

## Getting Started
This is a short guide for getting communication up and running with your
PROFIBUS peripheral:

1. Find the GSD (generic station description) file for your peripheral.
   Usually, the manufacturer offers these for download somewhere.
2. Run the `gsdtool` to set up the configuration and parameterization of your
   peripheral:
   ```bash
   cargo run -p gsdtool -- config-wizard path/to/peripheral.gsd
   ```
   The configuration wizard will walk you through all the settings you need to
   make.  At this stage, you also need to setup the modules of your peripheral.
   The wizard will then give you Rust code for configuring the peripheral
   options to your selected values.
3. Modify an example for your peripheral.  Update the peripheral address.  Then
   paste the `PeripheralOptions` block and I/O buffers that `gsdtool` emitted.
4. Run the example, ideally with `RUST_LOG=trace` to see bus communication.
   Hopefully, you should now be able to establish cyclic communication with
   your peripheral.

## License
Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or
  <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or
  <http://opensource.org/licenses/MIT>)

at your option.

### Contribution
Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall
be dual licensed as above, without any additional terms or conditions.

[PROFIBUS-DP]: https://en.wikipedia.org/wiki/Profibus
