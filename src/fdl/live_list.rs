#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StationDescription {
    pub address: crate::Address,
    pub state: super::ResponseState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StationEvent {
    Discovered(StationDescription),
    Lost(crate::Address),
}

#[derive(Debug, Clone)]
pub struct LiveList {
    stations: bitvec::BitArr!(for 128),
    cursor: crate::Address,
    pending_event: Option<StationEvent>,
    current_address_done: bool,
}

impl Default for LiveList {
    fn default() -> Self {
        Self::new()
    }
}

impl LiveList {
    pub fn new() -> Self {
        Self {
            stations: bitvec::array::BitArray::ZERO,
            cursor: 0,
            pending_event: None,
            current_address_done: false,
        }
    }

    pub fn iter_stations(&self) -> impl DoubleEndedIterator<Item = crate::Address> + '_ {
        self.stations.iter_ones().map(|a| u8::try_from(a).unwrap())
    }

    pub fn take_last_event(&mut self) -> Option<StationEvent> {
        self.pending_event.take()
    }
}

impl crate::fdl::FdlApplication for LiveList {
    fn transmit_telegram(
        &mut self,
        now: crate::time::Instant,
        fdl: &super::FdlActiveStation,
        tx: super::TelegramTx,
        high_prio_only: super::HighPrioOnly,
    ) -> Option<super::TelegramTxResponse> {
        let this_station = fdl.parameters().address;
        let address = self.cursor;

        if self.current_address_done {
            self.current_address_done = false;
            if self.cursor < 125 {
                self.cursor += 1;
            } else {
                self.cursor = 0;
            }
            None
        } else {
            Some(tx.send_fdl_status_request(address, this_station))
        }
    }

    fn receive_reply(
        &mut self,
        now: crate::time::Instant,
        fdl: &super::FdlActiveStation,
        addr: u8,
        telegram: super::Telegram,
    ) {
        self.current_address_done = true;
        let event = if !self.stations.get(usize::from(addr)).unwrap() {
            self.stations.set(usize::from(addr), true);

            if let super::Telegram::Data(super::DataTelegram {
                h:
                    super::DataTelegramHeader {
                        fc: super::FunctionCode::Response { state, status },
                        ..
                    },
                ..
            }) = telegram
            {
                Some(StationEvent::Discovered(StationDescription {
                    address: addr,
                    state,
                }))
            } else {
                None
            }
        } else {
            // We know this station already, so no event.
            None
        };

        self.pending_event = event;
    }

    fn handle_timeout(
        &mut self,
        now: crate::time::Instant,
        fdl: &super::FdlActiveStation,
        addr: u8,
    ) {
        self.current_address_done = true;
        if *self.stations.get(usize::from(addr)).unwrap() {
            self.pending_event = Some(StationEvent::Lost(addr));
            self.stations.set(usize::from(addr), false);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{LiveList, StationDescription, StationEvent};
    use crate::fdl;
    use crate::phy;
    use crate::phy::ProfibusPhy;

    struct LiveListUnderTest {
        control_addr: u8,
        timestep: crate::time::Duration,
        phy_control: phy::SimulatorPhy,
        phy_active: phy::SimulatorPhy,
        active_station: fdl::FdlActiveStation,
        live_list: LiveList,
    }

    impl LiveListUnderTest {
        fn new(addr: crate::Address) -> Self {
            let baud = crate::Baudrate::B19200;
            let control_addr = 15;
            let timestep = crate::time::Duration::from_micros(100);

            let phy_control = phy::SimulatorPhy::new(baud, "phy#control");
            let phy_active = phy_control.duplicate("phy#ut");

            let mut active_station = fdl::FdlActiveStation::new(
                crate::fdl::ParametersBuilder::new(addr, baud)
                    .highest_station_address(16)
                    .slot_bits(300)
                    .build(),
            );

            crate::test_utils::with_active_addr(active_station.parameters().address, || {
                active_station.set_online();
            });

            Self {
                control_addr,
                timestep,
                phy_control,
                phy_active,
                active_station,
                live_list: LiveList::new(),
            }
        }

        fn wait_for_matching<F: FnMut(fdl::Telegram) -> bool>(&mut self, f: F) {
            for now in self.phy_control.iter_until_matching(self.timestep, f) {
                crate::test_utils::set_log_timestamp(now);
                crate::test_utils::with_active_addr(
                    self.active_station.parameters().address,
                    || {
                        self.active_station
                            .poll(now, &mut self.phy_active, &mut self.live_list);
                    },
                );
            }
        }

        fn advance_bus_time_min_tsdr(&mut self) -> crate::time::Instant {
            let now = self.phy_control.advance_bus_time_min_tsdr();
            crate::test_utils::set_log_timestamp(now);
            crate::test_utils::with_active_addr(self.active_station.parameters().address, || {
                self.active_station
                    .poll(now, &mut self.phy_active, &mut self.live_list);
            });
            now
        }

        /// Drive the bus, responding to FDL status requests destined for addresses in
        /// `live_stations`, until `expected_event_count` events have been emitted by `LiveList`.
        fn drive_until_events(
            &mut self,
            live_stations: &[u8],
            expected_event_count: usize,
        ) -> Vec<StationEvent> {
            let mut events = Vec::with_capacity(expected_event_count);
            let active_station_address = self.active_station.parameters().address;

            while events.len() < expected_event_count {
                let mut scanned_addr: Option<u8> = None;
                self.wait_for_matching(|t| {
                    let fdl::Telegram::Data(dt) = t else {
                        return false;
                    };
                    if dt.is_fdl_status_request().is_none() {
                        return false;
                    }

                    assert_eq!(
                        dt.h.sa, active_station_address,
                        "Got telegram from very unexpected addres #{}? {dt:?}",
                        dt.h.sa
                    );
                    scanned_addr = Some(dt.h.da);
                    true
                });

                if let Some(evt) = self.live_list.take_last_event() {
                    events.push(evt);
                    if events.len() >= expected_event_count {
                        break;
                    }
                }

                let target =
                    scanned_addr.expect("wait_for_matching returned without setting scanned_addr");

                if live_stations.contains(&target) {
                    let now = self.advance_bus_time_min_tsdr();
                    crate::test_utils::with_active_addr(target, || {
                        self.phy_control.transmit_telegram(now, |tx| {
                            Some(tx.send_fdl_status_response(
                                active_station_address,
                                target,
                                fdl::ResponseState::Slave,
                                fdl::ResponseStatus::Ok,
                            ))
                        });
                    });
                }
            }
            events
        }

        fn get_live_list(&self) -> Vec<crate::Address> {
            let mut list: Vec<_> = self.live_list.iter_stations().collect();
            list.sort();
            list
        }
    }

    /// Test that the LiveList application discovers all stations on the bus.
    #[test]
    fn live_list_application() {
        crate::test_utils::prepare_test_logger();
        let mut ut = LiveListUnderTest::new(7);

        let live_stations = vec![3, 8, 11, 67];

        let events = ut.drive_until_events(&live_stations, live_stations.len());

        let mut discovered: Vec<_> = events
            .iter()
            .map(|e| match e {
                StationEvent::Discovered(StationDescription { address, state }) => {
                    assert_eq!(
                        *state,
                        fdl::ResponseState::Slave,
                        "expected Slave response state for #{address}"
                    );
                    *address
                }
                StationEvent::Lost(a) => panic!("unexpected Lost event for #{a} during discovery"),
            })
            .collect();
        discovered.sort();

        assert_eq!(discovered, live_stations);
        assert_eq!(ut.get_live_list(), live_stations);
    }

    /// Test that stations that stop responding are reported lost and removed from the live list.
    #[test]
    fn live_list_looses_stations() {
        crate::test_utils::prepare_test_logger();
        let mut ut = LiveListUnderTest::new(7);

        let test_stations = vec![3, 8, 11, 67];
        let removed = vec![8, 67];
        let kept = vec![3, 11];

        let _ = ut.drive_until_events(&test_stations, test_stations.len());

        assert_eq!(ut.get_live_list(), test_stations);

        let events = ut.drive_until_events(&kept, removed.len());

        let mut lost: Vec<u8> = events
            .iter()
            .map(|e| match e {
                StationEvent::Lost(a) => *a,
                StationEvent::Discovered(d) => {
                    panic!(
                        "unexpected Discovered event for #{} during loss phase",
                        d.address
                    )
                }
            })
            .collect();
        lost.sort();

        assert_eq!(lost, removed);
        assert_eq!(ut.get_live_list(), kept);
    }
}
