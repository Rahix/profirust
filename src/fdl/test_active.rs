use crate::fdl;
use crate::phy;
use crate::phy::ProfibusPhy;

struct FdlActiveUnderTest {
    control_addr: u8,
    timestep: crate::time::Duration,
    pub phy_control: phy::SimulatorPhy,
    phy_active: phy::SimulatorPhy,
    pub active_station: fdl::FdlActiveStation,
}

impl Default for FdlActiveUnderTest {
    fn default() -> Self {
        Self::new(7)
    }
}

impl FdlActiveUnderTest {
    pub fn new(addr: crate::Address) -> Self {
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

        crate::test_utils::set_active_addr(active_station.parameters().address);
        active_station.set_online();

        Self {
            control_addr,
            timestep,
            phy_control,
            phy_active,
            active_station,
        }
    }

    pub fn now(&self) -> crate::time::Instant {
        self.phy_control.bus_time()
    }

    pub fn fdl_param(&self) -> &fdl::Parameters {
        self.active_station.parameters()
    }

    pub fn do_fdl_active_station_cycle(&mut self) {
        crate::test_utils::set_active_addr(self.active_station.parameters().address);
        self.active_station
            .poll(self.phy_control.bus_time(), &mut self.phy_active, &mut ());
        crate::test_utils::set_active_addr(self.control_addr);
    }

    pub fn do_timestep(&mut self) {
        self.phy_control.advance_bus_time(self.timestep);
        crate::test_utils::set_log_timestamp(self.phy_control.bus_time());
        self.do_fdl_active_station_cycle();
    }

    pub fn wait_for_matching<F: FnMut(fdl::Telegram) -> bool>(
        &mut self,
        f: F,
    ) -> crate::time::Duration {
        let start = self.phy_control.bus_time();
        crate::test_utils::set_active_addr(self.control_addr);
        for now in self.phy_control.iter_until_matching(self.timestep, f) {
            crate::test_utils::set_log_timestamp(now);
            crate::test_utils::set_active_addr(self.active_station.parameters().address);
            self.active_station.poll(now, &mut self.phy_active, &mut ());
            crate::test_utils::set_active_addr(self.control_addr);
        }
        self.phy_control.bus_time() - start
    }

    pub fn advance_bus_time_min_tsdr(&mut self) {
        self.phy_control.advance_bus_time_min_tsdr();
        self.do_fdl_active_station_cycle();
    }

    pub fn advance_bus_time_sync_pause(&mut self) {
        self.advance_bus_time_bits(33);
        self.do_fdl_active_station_cycle();
    }

    pub fn advance_bus_time_bits(&mut self, bits: u32) {
        self.phy_control.advance_bus_time(self.bits_to_time(bits));
    }

    pub fn bits_to_time(&self, bits: u32) -> crate::time::Duration {
        self.active_station.parameters().bits_to_time(bits)
    }

    pub fn time_to_bits(&self, time: crate::time::Duration) -> u64 {
        self.active_station.parameters().baudrate.time_to_bits(time)
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

/// Test that an active station sends a claimed token twice before doing anything else.
#[test]
fn test_new_token_is_sent_twice() {
    crate::test_utils::prepare_test_logger();
    let mut fdl_ut = FdlActiveUnderTest::default();

    let addr = fdl_ut.fdl_param().address;
    fdl_ut.wait_for_matching(|t| {
        t == fdl::Telegram::Token(fdl::TokenTelegram { da: addr, sa: addr })
    });

    let mut got_second_token = false;
    fdl_ut.wait_for_matching(|t| {
        if !got_second_token {
            if t == fdl::Telegram::Token(fdl::TokenTelegram { da: addr, sa: addr }) {
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

/// Test that the active station waits for the appropriate amount of time before claiming the token
/// for itself on an idle bus.
#[rstest::rstest]
fn test_token_timeout(#[values(0, 1, 2, 7, 14)] addr: crate::Address) {
    crate::test_utils::prepare_test_logger();
    let mut fdl_ut = FdlActiveUnderTest::new(addr);

    fdl_ut.transmit_telegram(|tx| Some(tx.send_token_telegram(15, 15)));

    let start = fdl_ut.now();

    fdl_ut.wait_for_matching(|t| {
        t == fdl::Telegram::Token(fdl::TokenTelegram { da: addr, sa: addr })
    });

    let token_telegram_time = fdl_ut.bits_to_time(3 * 11);
    let timeout_measured = fdl_ut.now() - start - token_telegram_time;

    let expected_timeout =
        fdl_ut.bits_to_time(u32::from(fdl_ut.fdl_param().slot_bits) * (6 + 2 * u32::from(addr)));

    // Ensure the measured timeout also lies well before the timeout of the next address would
    // be reached.
    let expected_timeout_max = fdl_ut
        .bits_to_time(u32::from(fdl_ut.fdl_param().slot_bits) * (6 + 2 * u32::from(addr + 1)));

    log::info!(
        "Measured token timeout: {}us",
        timeout_measured.total_micros()
    );
    log::info!(
        "Expected token timeout: {}us < t < {}us",
        expected_timeout.total_micros(),
        expected_timeout_max.total_micros()
    );

    assert!(timeout_measured >= expected_timeout);
    assert!(timeout_measured <= expected_timeout_max);
}

/// Test active station FDL status response before initialization
#[test]
fn test_active_station_early_fdl_status() {
    crate::test_utils::prepare_test_logger();
    let mut fdl_ut = FdlActiveUnderTest::default();
    let addr = fdl_ut.fdl_param().address;

    fdl_ut.transmit_telegram(|tx| Some(tx.send_fdl_status_request(addr, 15)));

    fdl_ut.wait_for_matching(|t| {
        assert_eq!(
            t,
            fdl::Telegram::Data(fdl::DataTelegram {
                h: fdl::DataTelegramHeader {
                    da: 15,
                    sa: addr,
                    dsap: None,
                    ssap: None,
                    fc: fdl::FunctionCode::Response {
                        state: fdl::ResponseState::MasterNotReady,
                        status: fdl::ResponseStatus::Ok
                    },
                },
                pdu: &[],
            })
        );
        true
    });
}
