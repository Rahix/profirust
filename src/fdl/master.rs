#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum RingStatus {
    NotInRing,
    InRing,
    HasToken,
}

#[derive(Debug)]
pub struct FdlMaster {
    ring_status: RingStatus,
    last_transaction: crate::time::Instant,
}

impl FdlMaster {
    pub fn poll<'b, PHY: crate::phy::ProfibusPhy<'b>>(
        &mut self,
        timestamp: crate::time::Instant,
        phy: &mut PHY,
    ) {
        if (timestamp - self.last_transaction) > crate::time::Duration::from_secs(1) {
            log::warn!("Token lost!");
        }
    }
}
