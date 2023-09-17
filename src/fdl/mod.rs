//! FDL - Fieldbus Data Link
mod master;
mod parameters;
mod telegram;

#[cfg(test)]
mod tests;

pub use master::{ConnectivityState, FdlMaster};
pub use parameters::{Parameters, ParametersBuilder};
pub use telegram::{
    DataTelegram, DataTelegramHeader, FrameCountBit, FunctionCode, RequestType, ResponseState,
    ResponseStatus, ShortConfirmation, Telegram, TelegramTx, TelegramTxResponse, TokenTelegram,
};

/// The interface for application layer components.
///
/// Only one application layer component is permitted per FDL master.
pub trait FdlApplication {
    type Event;

    /// Possibly transmit a telegram.
    ///
    /// The FDL layer will know whether a reply is expected based on the telegram that is sent.  If
    /// a reply is received, `receive_reply()` will be called to handle it.  If no reply is
    /// received in Tsl time, `transmit_telegram()` is called again.  It should then retry
    /// transmission according to the retry count configured in `fdl.parameters().max_retry_limit`.
    ///
    /// When `transmit_telegram()` returns `None`, the FDL master will interpret this as end of
    /// cycle and will pass on the token.
    fn transmit_telegram(
        &mut self,
        now: crate::time::Instant,
        fdl: &FdlMaster,
        tx: TelegramTx,
        high_prio_only: bool,
    ) -> (Option<TelegramTxResponse>, Option<Self::Event>);

    /// Receive the reply for the telegram that was last transmitted.
    fn receive_reply(
        &mut self,
        now: crate::time::Instant,
        fdl: &FdlMaster,
        addr: u8,
        telegram: Telegram,
    ) -> Option<Self::Event>;

    /// Handle a timeout while waiting for a reply from the given address.
    fn handle_timeout(&mut self, now: crate::time::Instant, fdl: &FdlMaster, addr: u8);
}

// A sort of placeholder when no application is used.
impl FdlApplication for () {
    type Event = ();

    fn transmit_telegram(
        &mut self,
        now: crate::time::Instant,
        fdl: &FdlMaster,
        tx: TelegramTx,
        high_prio_only: bool,
    ) -> (Option<TelegramTxResponse>, Option<Self::Event>) {
        (None, None)
    }

    fn receive_reply(
        &mut self,
        now: crate::time::Instant,
        fdl: &FdlMaster,
        addr: u8,
        telegram: Telegram,
    ) -> Option<Self::Event> {
        None
    }

    fn handle_timeout(&mut self, now: crate::time::Instant, fdl: &FdlMaster, addr: u8) {
        // ignore
    }
}
