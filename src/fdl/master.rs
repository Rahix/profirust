pub struct FdlMaster {}

impl FdlMaster {
    pub fn poll<'a, PHY: crate::phy::ProfibusPhy<'a>>(
        &mut self,
        timestamp: crate::time::Instant,
        phy: &mut PHY,
    ) {
        todo!()
    }
}
