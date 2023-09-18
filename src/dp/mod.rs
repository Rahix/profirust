//! DP - Decentralized peripherals
//!
//! This module implements the DP application layer of PROFIBUS.  The main component is the
//! [`DpMaster`] type which manages the DP cyclic communication and peripheral lifecycle.
//!
//! Peripherals are represented as [`Peripheral`] objects which you need to construct using
//! [`PeripheralOptions`].  These options are best generated from the peripheral's GSD file using
//! the `gsdtool` that is part of the `profirust` project.
mod master;
mod peripheral;
mod peripheral_set;

pub use master::{DpEvents, DpMaster, DpMasterState, OperatingState};
pub use peripheral::{
    DiagnosticFlags, Peripheral, PeripheralDiagnostics, PeripheralEvent, PeripheralOptions,
};
pub use peripheral_set::{PeripheralHandle, PeripheralSet, PeripheralStorage};
