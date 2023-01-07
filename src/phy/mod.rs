#[cfg(feature = "phy-linux")]
mod linux;
#[cfg(feature = "phy-linux")]
pub use linux::LinuxRs485Phy;

pub type BufferHandle<'a> = managed::ManagedSlice<'a, u8>;

pub trait ProfibusPhy<'a> {
    /// Schedule transmission of some data.
    ///
    /// The first `length` bytes from `data` should be transmitted.
    ///
    /// **Important:** This function must not block on the actual transmission!
    fn schedule_tx<'b>(&'b mut self, data: BufferHandle<'a>, length: usize)
    where
        'a: 'b;

    /// Poll whether the ongoing transmission was completed.
    ///
    /// If completed, the buffer that was passed for transmission is returned.
    ///
    /// `poll_tx()` may panic when called again after returning the buffer.
    fn poll_tx(&mut self) -> Option<BufferHandle<'a>>;

    /// Schedule receival of data into the given buffer.
    fn schedule_rx(&'a mut self, data: BufferHandle<'a>);

    /// Peek at received data without touching the scheduled receival.
    ///
    /// If data was already received, peek into the receive buffer without releasing it.  The FDL
    /// may use this to check if a full telegram was received.
    fn peek_rx(&mut self) -> &[u8];

    /// Poll received data.
    ///
    /// This function must always immediately release the receive buffer and return a length of how
    /// much data was received.
    fn poll_rx(&mut self) -> (BufferHandle, usize);
}
