/// Status of the `LAS` (List of Active Stations)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LasState {
    /// We are listening to the first token rotation to discover all active stations.
    Discovery,
    /// We are listening to the second token rotation to verify correctness of the LAS.
    Verification,
    /// The LAS has been verified and is valid.  It will be updated live from now on.
    Valid,
}

impl LasState {
    /// Whether the LAS is valid.
    ///
    /// Returns `true` when `self == LasState::Valid`
    #[inline(always)]
    pub fn is_valid(self) -> bool {
        matches!(self, Self::Valid)
    }
}

/// Management of the token ring from the station's point of view
#[derive(Clone, PartialEq, Eq)]
pub struct TokenRing {
    /// `LAS` (List of Active Stations)
    active_stations: bitvec::BitArr!(for 128),

    /// Status of the `LAS`
    las_state: LasState,

    /// `TS` (This Station)
    this_station: crate::Address,

    /// `NS` (Next Station)
    ///
    /// The `next_station` is who we will forward the token to once we release it.
    ///
    /// There is always a `next_station`.  When no other active stations are known, we are our own
    /// `next_station`, so NS==TS.
    next_station: crate::Address,

    /// `PS` (Previous Station)
    ///
    /// The `previous_station` is who we will receive the token from.  At first, only a token from
    /// `previous_station` is accepted unless a station makes it clear that it is our new
    /// `previous_station` by passing the token to us a second time.
    ///
    /// There is always a `previous_station`.  When no other active stations are known, we are our
    /// own `previous_station`, so PS==TS.
    previous_station: crate::Address,
}

impl TokenRing {
    pub fn new(param: &crate::fdl::Parameters) -> Self {
        let mut active_stations = bitvec::array::BitArray::ZERO;
        // Mark ourselves in the list of active stations.
        active_stations.set(usize::from(param.address), true);

        Self {
            active_stations,
            las_state: LasState::Discovery,
            this_station: param.address,
            next_station: param.address,
            previous_station: param.address,
        }
    }

    pub fn iter_active_stations(&self) -> impl Iterator<Item = crate::Address> + '_ {
        self.active_stations
            .iter_ones()
            .map(|a| u8::try_from(a).unwrap())
    }

    pub fn ready_for_ring(&self) -> bool {
        self.las_state.is_valid()
    }

    pub fn this_station(&self) -> crate::Address {
        self.this_station
    }

    pub fn next_station(&self) -> u8 {
        self.next_station
    }

    pub fn previous_station(&self) -> u8 {
        self.previous_station
    }
}

impl core::fmt::Debug for TokenRing {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        todo!()
    }
}
