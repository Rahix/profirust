use crate::phy::ProfibusPhy;

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
fn two_masters_and_their_tokens() {
    crate::test_utils::prepare_test_logger();
    let baud = crate::Baudrate::B19200;

    let mut phy1 = crate::phy::SimulatorPhy::new(baud, "phy#2");
    let mut phy2 = phy1.duplicate("phy#7");

    let mut master1 = crate::fdl::FdlMaster::new(
        crate::fdl::ParametersBuilder::new(2, baud)
            .highest_station_address(16)
            .slot_bits(300)
            .build(),
    );
    let mut master2 = crate::fdl::FdlMaster::new(
        crate::fdl::ParametersBuilder::new(7, baud)
            .highest_station_address(16)
            .slot_bits(300)
            .build(),
    );

    crate::test_utils::set_active_addr(2);
    master1.set_online();

    crate::test_utils::set_active_addr(7);
    master2.set_online();

    let start = crate::time::Instant::ZERO;
    let mut now = start;
    while (now - start) < crate::time::Duration::from_millis(800) {
        crate::test_utils::set_log_timestamp(now);
        phy1.set_bus_time(now);

        crate::test_utils::set_active_addr(2);
        master1.poll(now, &mut phy1, &mut ());

        crate::test_utils::set_active_addr(7);
        master2.poll(now, &mut phy2, &mut ());

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
