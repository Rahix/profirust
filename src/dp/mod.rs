mod master;
mod peripheral;

pub use master::{DpMaster, PeripheralHandle, PeripheralStorage};
pub use peripheral::{DiagnosticFlags, Peripheral, PeripheralDiagnostics, PeripheralOptions};
