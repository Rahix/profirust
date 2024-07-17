use crate::fdl;
use crate::phy;
use crate::phy::ProfibusPhy;

struct FdlMasterUnderTest {
    control_addr: u8,
    timestep: crate::time::Duration,
    phy_control: phy::SimulatorPhy,
    phy_master: phy::SimulatorPhy,
    master: fdl::FdlMaster,
}

impl FdlMasterUnderTest {
    pub fn new() -> Self {
        let baud = crate::Baudrate::B19200;
        let addr = 7;
        let control_addr = 15;
        let timestep = crate::time::Duration::from_micros(100);

        let phy_control = phy::SimulatorPhy::new(baud, "phy#control");
        let phy_master = phy_control.duplicate("phy#ut");

        let mut master = fdl::FdlMaster::new(
            crate::fdl::ParametersBuilder::new(addr, baud)
                .highest_station_address(16)
                .slot_bits(300)
                .build(),
        );

        crate::test_utils::set_active_addr(master.parameters().address);
        master.set_online();

        Self {
            control_addr,
            timestep,
            phy_control,
            phy_master,
            master,
        }
    }

    pub fn do_master_cycle(&mut self) {
        crate::test_utils::set_active_addr(self.master.parameters().address);
        self.master
            .poll(self.phy_control.bus_time(), &mut self.phy_master, &mut ());
        crate::test_utils::set_active_addr(self.control_addr);
    }

    pub fn do_timestep(&mut self) {
        self.phy_control.advance_bus_time(self.timestep);
        crate::test_utils::set_log_timestamp(self.phy_control.bus_time());
        self.do_master_cycle();
    }

    pub fn wait_for_matching<F: FnMut(fdl::Telegram) -> bool>(
        &mut self,
        f: F,
    ) -> crate::time::Duration {
        let start = self.phy_control.bus_time();
        crate::test_utils::set_active_addr(self.control_addr);
        for now in self.phy_control.iter_until_matching(self.timestep, f) {
            crate::test_utils::set_log_timestamp(now);
            crate::test_utils::set_active_addr(self.master.parameters().address);
            self.master.poll(now, &mut self.phy_master, &mut ());
            crate::test_utils::set_active_addr(self.control_addr);
        }
        self.phy_control.bus_time() - start
    }

    pub fn advance_bus_time_min_tsdr(&mut self) {
        self.phy_control.advance_bus_time_min_tsdr();
        self.do_master_cycle();
    }

    pub fn advance_bus_time_bits(&mut self, bits: u32) {
        self.phy_control.advance_bus_time(self.bits_to_time(bits));
    }

    pub fn bits_to_time(&self, bits: u32) -> crate::time::Duration {
        self.master.parameters().bits_to_time(bits)
    }

    pub fn time_to_bits(&self, time: crate::time::Duration) -> u64 {
        self.master.parameters().baudrate.time_to_bits(time)
    }

    pub fn transmit_telegram<F>(&mut self, f: F) -> Option<fdl::TelegramTxResponse>
    where
        F: FnOnce(crate::fdl::TelegramTx) -> Option<fdl::TelegramTxResponse>,
    {
        let now = self.phy_control.bus_time();
        self.phy_control.transmit_telegram(now, f)
    }

    pub fn wait_transmission(&mut self) {
        while self
            .phy_control
            .poll_transmission(self.phy_control.bus_time())
        {
            self.do_timestep();
        }
    }

    pub fn assert_idle_time(&mut self, time: crate::time::Duration) {
        let timeout = self.phy_control.bus_time() + time;
        while self.phy_control.bus_time() < timeout {
            self.do_timestep();
            if self
                .phy_control
                .poll_pending_received_bytes(self.phy_control.bus_time())
                != 0
            {
                panic!("Idle time assertion failed!");
            }
        }
    }

    pub fn assert_idle_bits(&mut self, bits: u32) {
        self.assert_idle_time(self.bits_to_time(bits));
    }
}

/// Ensure proper token timeout.
#[rstest::rstest]
fn test_token_timeout(#[values(0, 1, 7, 14)] addr: u8) {
    crate::test_utils::prepare_test_logger();
    let baud = crate::Baudrate::B19200;
    let mut phy0 = crate::phy::SimulatorPhy::new(baud, "phy#0");
    let mut phy7 = phy0.duplicate("phy#7");

    let mut master7 = crate::fdl::FdlMaster::new(
        crate::fdl::ParametersBuilder::new(addr, baud)
            .highest_station_address(16)
            .slot_bits(300)
            .build(),
    );

    let start = crate::time::Instant::ZERO;
    let mut now = start;

    crate::test_utils::set_active_addr(addr);
    master7.set_online();

    crate::test_utils::set_active_addr(15);
    phy0.transmit_telegram(now, |tx| Some(tx.send_token_telegram(15, 15)));

    let mut new_token_time = None;
    while now.total_millis() < 800 {
        crate::test_utils::set_log_timestamp(now);
        phy0.set_bus_time(now);

        crate::test_utils::set_active_addr(addr);
        master7.poll(now, &mut phy7, &mut ());

        crate::test_utils::set_active_addr(15);
        if !phy0.poll_transmission(now) {
            phy0.receive_telegram(now, |t| match t {
                crate::fdl::Telegram::Token(crate::fdl::TokenTelegram { da, sa }) => {
                    if new_token_time.is_none() {
                        new_token_time = Some(now);
                        assert_eq!(da, addr);
                        assert_eq!(sa, addr);
                    }
                }
                crate::fdl::Telegram::Data(_) => assert!(new_token_time.is_some()),
                crate::fdl::Telegram::ShortConfirmation(_) => assert!(new_token_time.is_some()),
            });
        }

        now += crate::time::Duration::from_micros(100);
    }

    assert!(master7.is_in_ring());

    let timeout_start = start + baud.bits_to_time(3 * 11);
    let timeout_measured = new_token_time.expect("never reached token timeout?")
        - timeout_start
        - baud.bits_to_time(3 * 11);

    let expected_timeout =
        baud.bits_to_time(u32::from(master7.parameters().slot_bits) * (6 + 2 * u32::from(addr)));

    // Ensure the measured timeout also lies well before the timeout of the next address would
    // be reached.
    let expected_timeout_max = baud
        .bits_to_time(u32::from(master7.parameters().slot_bits) * (6 + 2 * u32::from(addr + 1)));

    log::info!(
        "Measured token timeout: {}us",
        timeout_measured.total_micros()
    );
    log::info!(
        "Expected token timeout: {}us - {}us",
        expected_timeout.total_micros(),
        expected_timeout_max.total_micros()
    );

    assert!(timeout_measured >= expected_timeout);
    assert!(timeout_measured <= expected_timeout_max);
}

#[test]
fn big_bus() {
    crate::test_utils::prepare_test_logger();
    let baud = crate::Baudrate::B19200;

    let actives_addr = vec![2, 7, 13, 24];
    let passives_addr = vec![3, 9, 10, 16, 18, 22, 26, 28];

    let phy = crate::phy::SimulatorPhy::new(baud, "phy#main");

    let mut actives: Vec<_> = actives_addr
        .iter()
        .copied()
        .map(|addr| {
            let phy = phy.duplicate(format!("phy#{addr}").leak());
            let mut fdl = crate::fdl::FdlMaster::new(
                crate::fdl::ParametersBuilder::new(addr, baud)
                    .highest_station_address(30)
                    .slot_bits(300)
                    .build(),
            );
            crate::test_utils::set_active_addr(addr);
            fdl.set_online();
            (addr, phy, fdl)
        })
        .collect();

    let mut passives: Vec<_> = passives_addr
        .iter()
        .copied()
        .map(|addr| {
            let phy = phy.duplicate(format!("phy#{addr}").leak());
            #[allow(unused_mut)]
            let mut fdl = crate::fdl::FdlMaster::new(
                crate::fdl::ParametersBuilder::new(addr, baud)
                    .highest_station_address(30)
                    .slot_bits(300)
                    .build(),
            );
            crate::test_utils::set_active_addr(addr);
            // TODO: Once passive mode is supported, enable: fdl.set_passive();
            (addr, phy, fdl)
        })
        .collect();

    let start = crate::time::Instant::ZERO;
    let mut now = start;
    while (now - start) < crate::time::Duration::from_millis(3200) {
        crate::test_utils::set_log_timestamp(now);
        phy.set_bus_time(now);

        for (addr, phy, fdl) in actives.iter_mut() {
            crate::test_utils::set_active_addr(*addr);
            fdl.poll(now, phy, &mut ());
        }

        for (addr, phy, fdl) in passives.iter_mut() {
            crate::test_utils::set_active_addr(*addr);
            fdl.poll(now, phy, &mut ());
        }

        now += crate::time::Duration::from_micros(100);
    }

    for (addr, _, fdl) in actives.iter() {
        assert!(
            fdl.is_in_ring(),
            "station #{addr} is not in the token ring!"
        );

        for i in 0..126 {
            // TODO: passives are also live
            assert_eq!(
                fdl.check_address_live(i),
                actives_addr.contains(&i),
                "wrong liveness of address #{i} reported by master #{addr}"
            );
        }
    }
}

#[test]
fn master_dropping_from_bus() {
    crate::test_utils::prepare_test_logger();
    let baud = crate::Baudrate::B19200;

    let mut actives_addr = vec![2, 7, 13, 24];

    let phy = crate::phy::SimulatorPhy::new(baud, "phy#main");

    let mut actives: Vec<_> = actives_addr
        .iter()
        .copied()
        .map(|addr| {
            let phy = phy.duplicate(format!("phy#{addr}").leak());
            let mut fdl = crate::fdl::FdlMaster::new(
                crate::fdl::ParametersBuilder::new(addr, baud)
                    .highest_station_address(30)
                    .slot_bits(300)
                    .build(),
            );
            crate::test_utils::set_active_addr(addr);
            fdl.set_online();
            (addr, phy, fdl)
        })
        .collect();

    let start = crate::time::Instant::ZERO;
    let mut now = start;
    while (now - start) < crate::time::Duration::from_millis(3200) {
        crate::test_utils::set_log_timestamp(now);
        phy.set_bus_time(now);

        for (addr, phy, fdl) in actives.iter_mut() {
            crate::test_utils::set_active_addr(*addr);
            fdl.poll(now, phy, &mut ());
        }

        if (now - start) >= crate::time::Duration::from_millis(2100) && actives_addr.contains(&7) {
            let x = actives_addr.remove(1);
            log::info!("Dropped station #{x} from the bus!");
            actives[1].2.set_offline();
            actives.remove(1);
        }

        now += crate::time::Duration::from_micros(100);
    }

    for (addr, _, fdl) in actives.iter() {
        assert!(
            fdl.is_in_ring(),
            "station #{addr} is not in the token ring!"
        );

        for i in 0..126 {
            // TODO: passives are also live
            assert_eq!(
                fdl.check_address_live(i),
                actives_addr.contains(&i),
                "wrong liveness of address #{i} reported by master #{addr}"
            );
        }
    }
}

/// Test that the FDL master detects when the token was not received.
///
/// In this case it should resend the token a second time before assuming the master is dead.
#[ignore = "currently failing"]
#[test]
fn test_token_not_received() {
    crate::test_utils::prepare_test_logger();
    let mut fdl_ut = FdlMasterUnderTest::new();

    fdl_ut.wait_for_matching(|t| {
        matches!(
            t,
            fdl::Telegram::Data(fdl::DataTelegram {
                h: fdl::DataTelegramHeader {
                    da: 15,
                    sa: 7,
                    dsap: None,
                    ssap: None,
                    fc: fdl::FunctionCode::Request {
                        req: fdl::RequestType::FdlStatus,
                        ..
                    },
                },
                pdu: &[],
            })
        )
    });

    fdl_ut.advance_bus_time_min_tsdr();
    fdl_ut.transmit_telegram(|tx| {
        Some(tx.send_fdl_status_response(
            7,
            15,
            fdl::ResponseState::MasterWithoutToken,
            fdl::ResponseStatus::Ok,
        ))
    });

    fdl_ut.wait_for_matching(|t| t == fdl::Telegram::Token(fdl::TokenTelegram { da: 15, sa: 7 }));

    // The master should retry sending the token again
    fdl_ut.wait_for_matching(|t| t == fdl::Telegram::Token(fdl::TokenTelegram { da: 15, sa: 7 }));

    // And then, after still not receiving an answer, it should go to the next master which it is
    // itself in this case.
    fdl_ut.wait_for_matching(|t| t == fdl::Telegram::Token(fdl::TokenTelegram { da: 7, sa: 7 }));
}

#[ignore = "currently failing"]
#[test]
fn test_token_not_accepted_from_random() {
    crate::test_utils::prepare_test_logger();
    let mut fdl_ut = FdlMasterUnderTest::new();

    fdl_ut.wait_for_matching(|t| {
        matches!(
            t,
            fdl::Telegram::Data(fdl::DataTelegram {
                h: fdl::DataTelegramHeader {
                    da: 15,
                    sa: 7,
                    dsap: None,
                    ssap: None,
                    fc: fdl::FunctionCode::Request {
                        req: fdl::RequestType::FdlStatus,
                        ..
                    },
                },
                pdu: &[],
            })
        )
    });

    fdl_ut.advance_bus_time_min_tsdr();
    fdl_ut.transmit_telegram(|tx| {
        Some(tx.send_fdl_status_response(
            7,
            15,
            fdl::ResponseState::MasterWithoutToken,
            fdl::ResponseStatus::Ok,
        ))
    });

    fdl_ut.wait_for_matching(|t| t == fdl::Telegram::Token(fdl::TokenTelegram { da: 15, sa: 7 }));

    fdl_ut.advance_bus_time_min_tsdr();
    fdl_ut.transmit_telegram(|tx| Some(tx.send_token_telegram(4, 15)));
    fdl_ut.wait_transmission();

    fdl_ut.advance_bus_time_min_tsdr();
    fdl_ut.transmit_telegram(|tx| Some(tx.send_token_telegram(7, 4)));
    fdl_ut.wait_transmission();

    // FdlMaster must not accept the token the first time
    fdl_ut.assert_idle_bits(66);

    fdl_ut.transmit_telegram(|tx| Some(tx.send_token_telegram(7, 4)));
    fdl_ut.wait_transmission();

    fdl_ut.wait_for_matching(|t| t == fdl::Telegram::Token(fdl::TokenTelegram { da: 15, sa: 7 }));
}

#[ignore = "currently failing"]
#[test]
fn test_new_token_is_sent_twice() {
    crate::test_utils::prepare_test_logger();
    let mut fdl_ut = FdlMasterUnderTest::new();

    fdl_ut.wait_for_matching(|t| t == fdl::Telegram::Token(fdl::TokenTelegram { da: 7, sa: 7 }));

    let mut got_second_token = false;
    fdl_ut.wait_for_matching(|t| {
        if !got_second_token {
            if t == fdl::Telegram::Token(fdl::TokenTelegram { da: 7, sa: 7 }) {
                got_second_token = true;
            } else {
                panic!("Got an unexpected telegram instead of second token: {t:?}");
            }
            false
        } else {
            true
        }
    });
}
