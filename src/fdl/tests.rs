use crate::phy::ProfibusPhy;

/// Ensure proper token timeout.
#[rstest::rstest]
fn test_token_timeout(#[values(0, 1, 7, 14)] addr: u8) {
    crate::test_utils::prepare_test_logger();
    let baud = crate::Baudrate::B19200;
    let mut phy0 = crate::phy::SimulatorPhy::new(baud, "phy#0");
    let mut phy7 = phy0.duplicate("phy#7");

    let mut per7 = crate::dp::PeripheralSet::new(vec![]);
    let mut master7 = crate::fdl::FdlMaster::new(crate::fdl::Parameters {
        address: addr,
        baudrate: baud,
        highest_station_address: 16,
        slot_bits: 300,
        ..Default::default()
    });

    crate::test_utils::set_active_addr(addr);
    master7.enter_operate();

    crate::test_utils::set_active_addr(0);
    phy0.transmit_telegram(|tx| Some(tx.send_token_telegram(15, 15)));

    let start = crate::time::Instant::ZERO;
    let mut now = start;
    let mut new_token_time = None;
    while now.total_millis() < 800 {
        crate::test_utils::set_log_timestamp(now);
        phy0.set_bus_time(now);

        crate::test_utils::set_active_addr(addr);
        master7.poll(now, &mut phy7, &mut per7);

        crate::test_utils::set_active_addr(0);
        if !phy0.is_transmitting() {
            phy0.receive_telegram(|t| match t {
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
fn two_masters_and_their_tokens() {
    crate::test_utils::prepare_test_logger();
    let baud = crate::Baudrate::B19200;

    let mut phy1 = crate::phy::SimulatorPhy::new(baud, "phy#2");
    let mut phy2 = phy1.duplicate("phy#7");

    let mut per1 = crate::dp::PeripheralSet::new(vec![]);
    let mut master1 = crate::fdl::FdlMaster::new(crate::fdl::Parameters {
        address: 2,
        baudrate: baud,
        highest_station_address: 16,
        slot_bits: 300,
        ..Default::default()
    });
    let mut per2 = crate::dp::PeripheralSet::new(vec![]);
    let mut master2 = crate::fdl::FdlMaster::new(crate::fdl::Parameters {
        address: 7,
        baudrate: baud,
        highest_station_address: 16,
        slot_bits: 300,
        ..Default::default()
    });

    crate::test_utils::set_active_addr(2);
    master1.enter_operate();

    crate::test_utils::set_active_addr(7);
    master2.enter_operate();

    let start = crate::time::Instant::ZERO;
    let mut now = start;
    while (now - start) < crate::time::Duration::from_millis(800) {
        crate::test_utils::set_log_timestamp(now);
        phy1.set_bus_time(now);

        crate::test_utils::set_active_addr(2);
        master1.poll(now, &mut phy1, &mut per1);

        crate::test_utils::set_active_addr(7);
        master2.poll(now, &mut phy2, &mut per2);

        now += crate::time::Duration::from_micros(100);
    }

    assert!(master1.is_in_ring());
    assert!(master2.is_in_ring());

    for i in 0..24 {
        assert_eq!(
            master1.check_address_live(i),
            i == 2 || i == 7,
            "wrong liveness of address {i} reported by master1(#2)"
        );
        assert_eq!(
            master2.check_address_live(i),
            i == 2 || i == 7,
            "wrong liveness of address {i} reported by master2(#7)"
        );
    }
}
