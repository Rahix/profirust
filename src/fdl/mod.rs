//! FDL - Fieldbus Data Link
mod master;
mod parameters;
mod peripheral_set;
mod telegram;

#[cfg(test)]
mod tests;

pub use master::{FdlMaster, OperatingState};
pub use parameters::Parameters;
pub use peripheral_set::{PeripheralHandle, PeripheralSet, PeripheralStorage};
pub use telegram::{
    DataTelegram, DataTelegramHeader, FrameCountBit, FunctionCode, RequestType, ResponseState,
    ResponseStatus, ShortConfirmation, Telegram, TelegramTx, TelegramTxResponse, TokenTelegram,
};
