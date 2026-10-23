use spike_e1_rust::ipv4::{ConfigurationError, Ipv4LinkMode, Ipv4Network};
use spike_e1_rust::{EndpointRef, Engine, Ipv4Address, MacAddress};
use spike_e1_rust::{EngineError, ObservableEvent};

#[test]
fn slash31_switch_cannot_be_a_peer_even_with_ipv4_metadata() {
    let mut e = Engine::with_default_trace_capacity();
    e.create_host("h".into(), vec!["p".into()]).unwrap();
    e.create_l2_switch("sw".into(), vec!["p".into()]).unwrap();
    for (name, last) in [("h", 0), ("sw", 1)] {
        e.set_interface_ipv4_mode(EndpointRef::new(name, "p"), Ipv4LinkMode::PointToPoint)
            .unwrap();
        e.configure_interface(
            EndpointRef::new(name, "p"),
            MacAddress([2, 0, 0, 0, 0, last + 1]),
            Some((Ipv4Address([172, 16, 0, last]), 31)),
            true,
        )
        .unwrap();
    }
    e.connect_link(
        "wire".into(),
        EndpointRef::new("h", "p"),
        EndpointRef::new("sw", "p"),
    )
    .unwrap();
    assert_eq!(
        e.validate_ipv4_topology(),
        Err(EngineError::Ipv4Configuration(
            ConfigurationError::InvalidPointToPointTopology
        ))
    );
    assert!(e.get_trace_snapshot().records.is_empty());
}

fn pair(peer_last: u8, peer_prefix: u8, peer_mode: bool) -> Engine {
    let mut e = Engine::with_default_trace_capacity();
    for (name, last, prefix, declared) in [
        ("h0", 0, 31, true),
        ("h1", peer_last, peer_prefix, peer_mode),
    ] {
        e.create_host(name.into(), vec!["p".into()]).unwrap();
        if declared {
            e.set_interface_ipv4_mode(EndpointRef::new(name, "p"), Ipv4LinkMode::PointToPoint)
                .unwrap();
        }
        e.configure_interface(
            EndpointRef::new(name, "p"),
            MacAddress([2, 0, 0, 0, 0, last + 1]),
            Some((Ipv4Address([172, 16, 0, last]), prefix)),
            true,
        )
        .unwrap();
    }
    e.connect_link(
        "wire".into(),
        EndpointRef::new("h0", "p"),
        EndpointRef::new("h1", "p"),
    )
    .unwrap();
    e
}

#[test]
fn slash31_direct_pair_accepts_both_endpoints_and_bidirectional_existing_ping() {
    let mut e = pair(1, 31, true);
    e.validate_ipv4_topology().unwrap();
    for (source, target, id) in [("h0", 1, 1), ("h1", 0, 2)] {
        e.send_ping(source.into(), Ipv4Address([172, 16, 0, target]), id, 1)
            .unwrap();
        e.run_until_quiescent(100).unwrap();
    }
    let trace = e.get_trace_snapshot();
    for host in ["h0", "h1"] {
        assert_eq!(trace.records.iter().filter(|r| matches!(&r.event,ObservableEvent::PingCompleted { host: h,.. } if h.0==host)).count(),1);
    }
    assert!(
        !trace
            .records
            .iter()
            .any(|r| matches!(&r.event, ObservableEvent::PingFailed { .. }))
    );
}

#[test]
fn slash31_rejects_equal_address_wrong_pair_prefix_and_missing_mode_atomically() {
    for (last, prefix, mode) in [(0, 31, true), (2, 31, true), (1, 30, true), (1, 31, false)] {
        let mut e = pair(last, prefix, mode);
        let before = e.get_trace_snapshot();
        assert_eq!(
            e.validate_ipv4_topology(),
            Err(EngineError::Ipv4Configuration(
                ConfigurationError::InvalidPointToPointTopology
            ))
        );
        assert_eq!(
            e.send_ping("h0".into(), Ipv4Address([172, 16, 0, 1]), 1, 1),
            Err(EngineError::Ipv4Configuration(
                ConfigurationError::InvalidPointToPointTopology
            ))
        );
        assert_eq!(e.get_trace_snapshot(), before);
        assert_eq!(e.now().0, 0);
    }
}

#[test]
fn slash31_rejects_switch_path_and_revalidates_after_disconnect_and_reconnect() {
    let mut e = pair(1, 31, true);
    e.disconnect_link(&"wire".into()).unwrap();
    assert!(e.validate_ipv4_topology().is_err());
    e.create_l2_switch("sw".into(), vec!["a".into(), "b".into()])
        .unwrap();
    e.connect_link(
        "left".into(),
        EndpointRef::new("h0", "p"),
        EndpointRef::new("sw", "a"),
    )
    .unwrap();
    e.connect_link(
        "right".into(),
        EndpointRef::new("h1", "p"),
        EndpointRef::new("sw", "b"),
    )
    .unwrap();
    assert!(e.validate_ipv4_topology().is_err());
    e.disconnect_link(&"left".into()).unwrap();
    e.disconnect_link(&"right".into()).unwrap();
    e.connect_link(
        "wire".into(),
        EndpointRef::new("h0", "p"),
        EndpointRef::new("h1", "p"),
    )
    .unwrap();
    e.validate_ipv4_topology().unwrap();
    e.set_interface_admin_state(EndpointRef::new("h1", "p"), false)
        .unwrap();
    e.validate_ipv4_topology().unwrap();
    assert_eq!(
        e.interface_is_operational(&EndpointRef::new("h0", "p"))
            .unwrap(),
        true
    );
    assert_eq!(
        e.interface_is_operational(&EndpointRef::new("h1", "p"))
            .unwrap(),
        false
    );
}

#[test]
fn slash31_router_attachment_static_route_and_busy_mode() {
    use spike_e1_rust::ipv4::Ipv4InterfaceConfig;
    use spike_e1_rust::router_engine::RouterPortConfig;
    let mut e = pair(1, 31, true);
    let mut first = RouterPortConfig::new(
        "a".into(),
        MacAddress([2, 0, 0, 0, 0, 3]),
        Ipv4InterfaceConfig::new(Ipv4Address([10, 0, 0, 0]), 31).unwrap(),
        true,
    );
    first.mode = Ipv4LinkMode::PointToPoint;
    e.create_router(
        "r".into(),
        vec![
            first,
            RouterPortConfig::new(
                "b".into(),
                MacAddress([2, 0, 0, 0, 0, 4]),
                Ipv4InterfaceConfig::new(Ipv4Address([192, 168, 1, 1]), 24).unwrap(),
                true,
            ),
        ],
    )
    .unwrap();
    assert!(e.validate_ipv4_topology().is_err()); // reserve /31 is incomplete until wired
    e.create_host("peer".into(), vec!["p".into()]).unwrap();
    e.set_interface_ipv4_mode(EndpointRef::new("peer", "p"), Ipv4LinkMode::PointToPoint)
        .unwrap();
    e.configure_interface(
        EndpointRef::new("peer", "p"),
        MacAddress([2, 0, 0, 0, 0, 5]),
        Some((Ipv4Address([10, 0, 0, 1]), 31)),
        true,
    )
    .unwrap();
    e.connect_link(
        "routerwire".into(),
        EndpointRef::new("r", "a"),
        EndpointRef::new("peer", "p"),
    )
    .unwrap();
    e.add_router_static_route(
        &"r".into(),
        Ipv4Network::new(Ipv4Address([203, 0, 113, 0]), 31).unwrap(),
        None,
        "b".into(),
        0,
    )
    .unwrap();
    e.validate_ipv4_topology().unwrap();
    assert_eq!(
        e.lookup_router_route(&"r".into(), Ipv4Address([10, 0, 0, 1]))
            .unwrap()
            .unwrap()
            .egress_interface
            .0,
        "a"
    );
    e.send_ping("h0".into(), Ipv4Address([172, 16, 0, 1]), 1, 1)
        .unwrap();
    assert_eq!(
        e.set_interface_ipv4_mode(EndpointRef::new("r", "a"), Ipv4LinkMode::MultiAccess),
        Err(EngineError::EngineBusy)
    );
    e.run_until_quiescent(100).unwrap();
    e.validate_ipv4_topology().unwrap();
}

#[test]
fn slash31_traffic_admission_rejects_incomplete_topology_without_any_effect() {
    let mut e = Engine::with_default_trace_capacity();
    e.create_host("h".into(), vec!["p".into()]).unwrap();
    e.configure_interface(
        EndpointRef::new("h", "p"),
        MacAddress([2, 0, 0, 0, 0, 1]),
        Some((Ipv4Address([172, 16, 0, 0]), 31)),
        true,
    )
    .unwrap();
    let before = e.get_trace_snapshot();
    assert!(
        e.send_ping("h".into(), Ipv4Address([172, 16, 0, 1]), 1, 1)
            .is_err()
    );
    assert_eq!(e.get_trace_snapshot(), before);
    assert_eq!(e.now().0, 0);
}

#[test]
fn slash31_global_gate_does_not_mutate_unrelated_ping_or_block_pending_link_failure() {
    let mut e = pair(1, 31, true);
    e.create_host("unused".into(), vec!["p".into()]).unwrap();
    e.configure_interface(
        EndpointRef::new("unused", "p"),
        MacAddress([2, 0, 0, 0, 0, 9]),
        Some((Ipv4Address([10, 0, 0, 0]), 31)),
        false,
    )
    .unwrap();
    let before = e.get_trace_snapshot();
    assert_eq!(
        e.send_ping("h0".into(), Ipv4Address([172, 16, 0, 1]), 7, 1),
        Err(EngineError::Ipv4Configuration(
            ConfigurationError::InvalidPointToPointTopology
        ))
    );
    assert_eq!(e.get_trace_snapshot(), before);
    e.configure_interface(
        EndpointRef::new("unused", "p"),
        MacAddress([2, 0, 0, 0, 0, 9]),
        Some((Ipv4Address([10, 0, 0, 1]), 24)),
        false,
    )
    .unwrap();
    e.send_ping("h0".into(), Ipv4Address([172, 16, 0, 1]), 7, 1)
        .unwrap(); // same key: rejected admission created no session
    e.step_events(1).unwrap(); // the initial deferred transmission is now on the wire
    e.disconnect_link(&"wire".into()).unwrap();
    assert_eq!(
        e.run_until_quiescent(100).unwrap().outcome,
        spike_e1_rust::ExecutionOutcome::Quiescent
    );
    assert!(e.get_trace_snapshot().records.iter().any(|r| matches!(
        &r.event,
        ObservableEvent::PacketDropped {
            reason: spike_e1_rust::DropReason::LinkDisconnectedInFlight,
            ..
        }
    )));
    assert_eq!(
        e.get_trace_snapshot()
            .records
            .iter()
            .filter(|r| matches!(&r.event,ObservableEvent::PingFailed { host,.. } if host.0=="h0"))
            .count(),
        1
    );
}

#[test]
fn slash31_router_to_router_pair_is_structural_not_a_host_special_case() {
    use spike_e1_rust::ipv4::Ipv4InterfaceConfig;
    use spike_e1_rust::router_engine::RouterPortConfig;
    let mut e = Engine::with_default_trace_capacity();
    for (name, last, lan) in [("r0", 0, 10), ("r1", 1, 20)] {
        let mut transit = RouterPortConfig::new(
            "transit".into(),
            MacAddress([2, 0, 0, 0, 1, last]),
            Ipv4InterfaceConfig::new(Ipv4Address([198, 51, 100, last]), 31).unwrap(),
            true,
        );
        transit.mode = Ipv4LinkMode::PointToPoint;
        e.create_router(
            name.into(),
            vec![
                transit,
                RouterPortConfig::new(
                    "lan".into(),
                    MacAddress([2, 0, 0, 0, 2, last]),
                    Ipv4InterfaceConfig::new(Ipv4Address([192, 168, lan, 1]), 24).unwrap(),
                    true,
                ),
            ],
        )
        .unwrap();
    }
    e.connect_link(
        "transit".into(),
        EndpointRef::new("r0", "transit"),
        EndpointRef::new("r1", "transit"),
    )
    .unwrap();
    e.validate_ipv4_topology().unwrap();
    assert!(
        e.interface_is_operational(&EndpointRef::new("r0", "transit"))
            .unwrap()
    );
    assert!(
        e.interface_is_operational(&EndpointRef::new("r1", "transit"))
            .unwrap()
    );
    assert_eq!(e.router_routes(&"r0".into()).unwrap().len(), 2);
    assert!(e.get_trace_snapshot().records.is_empty());
}
