<p align="center">
  <img src="img/logo-header.svg" alt="profirust"><br>
  <br>
  A PROFIBUS-DP communication stack written in Rust.
</p>

## What's this?
**profirust** is a pure-Rust [PROFIBUS-DP] communication stack.  PROFIBUS is an
industrial bus protocol used to communicate with field devices like remote I/O,
transducers, valves, drives, etc.

## Project Status
**profirust** is not yet in a state where it should be used in production.  It
is not yet fully fault tolerant so your applications may crash at the worst
time if unhandled bus states are entered.

At this time, **profirust** is developed as a spare time project.  If you are
interested in this project, help is gladly accepted in the following forms:

- Code Contributions
- Donation of PROFIBUS peripherals for testing purposes
- Funding of access to the needed IEC standards for improving compliance
- Reporting any kinds of issues encountered while using **profirust**

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
   The wizard will then give you the ident number and byte arrays for
   parameterization and configuration.
3. Modify an example for your peripheral.  Update the peripheral address, ident
   number, parameterization data, and configuration data.  You will also need
   to correctly set the sizes of the input and output process image buffers.
   Unfortunately, these sizes are not yet determined by `gsdtool`
   automatically.
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
