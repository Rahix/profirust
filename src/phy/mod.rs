#[cfg(feature = "phy-linux")]
mod linux;
#[cfg(feature = "phy-linux")]
pub use linux::LinuxRs485Phy;

#[cfg(feature = "phy-testbus")]
mod testbus;
#[cfg(feature = "phy-testbus")]
pub use testbus::TestBusPhy;

pub type BufferHandle<'a> = managed::ManagedSlice<'a, u8>;

pub trait ProfibusPhy {
    /// Check whether a transmission is currently ongoing.
    ///
    /// While this function returns `true`, calling any of the `transmit_*()` or `receive_*()`
    /// functions may panic.
    fn is_transmitting(&mut self) -> bool;

    /// Schedule transmission of some data.
    ///
    /// The data is written by the closure `f` into the buffer passed to it.  `f` then returns how
    /// many bytes were written.  Only this many bytes must be transmitted.
    ///
    /// **Important**: This function must not block on the actual transmission!
    ///
    /// ## Panics
    /// This function may panic when a transmission is already ongoing.
    fn transmit_data<F, R>(&mut self, f: F) -> R
    where
        F: FnOnce(&mut [u8]) -> (usize, R);

    /// Schedule transmission of a telegram.
    ///
    /// The closure `f` may (or may not) call one of the methods of [`TelegramTx`] to schedule
    /// transmission of a telegram.  This function returns `true` when a telegram was scheduled and
    /// `false` otherwise.
    ///
    /// **Important**: This function must not block on the actual transmission!
    ///
    /// ## Panics
    /// This function may panic when a transmission is already ongoing.
    fn transmit_telegram<F>(&mut self, f: F) -> bool
    where
        F: FnOnce(crate::fdl::TelegramTx) -> Option<crate::fdl::TelegramTxResponse>,
    {
        self.transmit_data(|buffer| {
            let ttx = crate::fdl::TelegramTx::new(buffer);
            let response = f(ttx);
            if let Some(response) = response {
                log::trace!("PHY TX {:?}", crate::fdl::Telegram::deserialize(buffer).unwrap().unwrap());
                (response.bytes_sent(), true)
            } else {
                (0, false)
            }
        })
    }

    /// Try receiving some data.
    ///
    /// The closure `f` will process all received data and return how many bytes should be dropped
    /// from the receive buffer.
    ///
    /// **Important**: This function must not block on the actually receiving data and should
    /// instead return an empty buffer if no data is available!
    ///
    /// ## Panics
    /// This function may panic when a transmission is ongoing.
    fn receive_data<F, R>(&mut self, f: F) -> R
    where
        F: FnOnce(&[u8]) -> (usize, R);

    /// Try receiving a telegram.
    ///
    /// When a full and correct telegram was received, the closure `f` is called to process it.
    ///
    /// **Important**: This function must not block on the actually receiving a telegram and should
    /// return `None` in case no full telegram was received yet!
    ///
    /// ## Panics
    /// This function may panic when a transmission is ongoing.
    fn receive_telegram<F, R>(&mut self, f: F) -> Option<R>
    where
        F: FnOnce(crate::fdl::Telegram) -> R,
    {
        self.receive_data(|buffer| {
            match crate::fdl::Telegram::deserialize(buffer) {
                // Discard all received data on error.
                Some(Err(_)) => (buffer.len(), None),
                // TODO: Only drop telegram length bytes instead of whole buffer.
                Some(Ok(telegram)) => {
                    log::trace!("PHY RX {:?}", telegram);
                    (buffer.len(), Some(f(telegram)))
                },
                // Don't drop any bytes yet if the telegram isn't complete.
                None => (0, None),
            }
        })
    }
}
