use spike_e1_rust::ipv4::ConfigurationError;
use spike_e1_rust::ipv4::{Ipv4InterfaceConfig, Ipv4Network};
use spike_e1_rust::router_engine::RouterPortConfig;
use spike_e1_rust::{DeviceRef, EndpointRef, Engine, Ipv4Address, MacAddress};
use spike_e1_rust::{EngineError, ExecutionOutcome};

fn ip(a: u8, b: u8, c: u8, d: u8) -> Ipv4Address {
    Ipv4Address([a, b, c, d])
}
fn ports() -> Vec<RouterPortConfig> {
    vec![
        RouterPortConfig::new(
            "a".into(),
            MacAddress([2, 0, 0, 0, 0, 1]),
            Ipv4InterfaceConfig::new(ip(192, 168, 1, 1), 24).unwrap(),
            true,
        ),
        RouterPortConfig::new(
            "b".into(),
            MacAddress([2, 0, 0, 0, 0, 2]),
            Ipv4InterfaceConfig::new(ip(172, 16, 1, 1), 24).unwrap(),
            true,
        ),
    ]
}
fn engine() -> Engine {
    let mut e = Engine::with_default_trace_capacity();
    e.create_router("r".into(), ports()).unwrap();
    e
}

#[test]
fn router_registration_is_atomic_and_reserves_existing_identifiers() {
    let mut e = engine();
    assert_eq!(e.router_routes(&"r".into()).unwrap().len(), 2);
    assert_eq!(
        e.interface_ipv4(&EndpointRef::new("r", "a")).unwrap(),
        Some((ip(192, 168, 1, 1), 24))
    );
    assert!(e.create_router("r".into(), ports()).is_err());
    assert!(e.create_router("bad".into(), vec![]).is_err());
    e.create_host("bad".into(), vec!["p".into()]).unwrap();
    let mut duplicate = ports();
    duplicate[1].name = "a".into();
    assert!(e.create_router("duplicate".into(), duplicate).is_err());
    e.create_host("duplicate".into(), vec!["p".into()]).unwrap();
    assert_eq!(e.now().0, 0);
    assert!(e.get_trace_snapshot().records.is_empty());
}

#[test]
fn router_lpm_uses_live_admin_and_link_generations_without_deleting_routes() {
    let mut e = engine();
    e.create_host("peer".into(), vec!["x".into(), "y".into()])
        .unwrap();
    e.add_router_static_route(
        &"r".into(),
        Ipv4Network::new(ip(10, 0, 0, 0), 8).unwrap(),
        Some(ip(172, 16, 1, 2)),
        "b".into(),
        0,
    )
    .unwrap();
    e.add_router_static_route(
        &"r".into(),
        Ipv4Network::new(ip(0, 0, 0, 0), 0).unwrap(),
        Some(ip(192, 168, 1, 2)),
        "a".into(),
        99,
    )
    .unwrap();
    assert!(
        e.lookup_router_route(&"r".into(), ip(10, 1, 1, 1))
            .unwrap()
            .is_none()
    );
    e.connect_link(
        "left".into(),
        EndpointRef::new("r", "a"),
        EndpointRef::new("peer", "x"),
    )
    .unwrap();
    let selected = |e: &Engine| {
        e.lookup_router_route(&DeviceRef::from("r"), ip(10, 1, 1, 1))
            .unwrap()
            .unwrap()
            .egress_interface
            .0
    };
    assert_eq!(selected(&e), "a");
    e.connect_link(
        "right".into(),
        EndpointRef::new("r", "b"),
        EndpointRef::new("peer", "y"),
    )
    .unwrap();
    assert_eq!(selected(&e), "b");
    e.set_interface_admin_state(EndpointRef::new("r", "b"), false)
        .unwrap();
    assert_eq!(selected(&e), "a");
    e.set_interface_admin_state(EndpointRef::new("r", "b"), true)
        .unwrap();
    assert_eq!(selected(&e), "b");
    // A peer's admin state does not change the local interface's RT-06 predicate.
    e.set_interface_admin_state(EndpointRef::new("peer", "y"), false)
        .unwrap();
    assert_eq!(selected(&e), "b");
    e.disconnect_link(&"right".into()).unwrap();
    assert_eq!(selected(&e), "a");
    e.connect_link(
        "right".into(),
        EndpointRef::new("r", "b"),
        EndpointRef::new("peer", "y"),
    )
    .unwrap();
    assert_eq!(selected(&e), "b");
    e.disconnect_link(&"right".into()).unwrap();
    e.disconnect_link(&"left".into()).unwrap();
    assert!(
        e.lookup_router_route(&"r".into(), ip(10, 1, 1, 1))
            .unwrap()
            .is_none()
    );
    assert_eq!(e.router_routes(&"r".into()).unwrap().len(), 4);
    assert!(e.get_trace_snapshot().records.is_empty());
    assert_eq!(e.now().0, 0);
}

#[test]
fn router_legacy_reconfiguration_synchronizes_connected_routes_and_rolls_back_errors() {
    let mut e = engine();
    e.create_host("peer".into(), vec!["p".into()]).unwrap();
    e.connect_link(
        "wire".into(),
        EndpointRef::new("r", "b"),
        EndpointRef::new("peer", "p"),
    )
    .unwrap();
    let r = "r".into();
    let ep = EndpointRef::new("r", "b");
    let mac = MacAddress([2, 0, 0, 0, 0, 2]);
    let before = e.router_routes(&r).unwrap();
    assert_eq!(
        e.configure_interface(ep.clone(), mac, Some((ip(192, 168, 1, 2), 24)), false),
        Err(EngineError::Ipv4Configuration(
            ConfigurationError::DuplicateConnectedPrefix
        ))
    );
    assert_eq!(e.router_routes(&r).unwrap(), before);
    assert!(e.interface_is_operational(&ep).unwrap());
    assert_eq!(
        e.interface_ipv4(&ep).unwrap(),
        Some((ip(172, 16, 1, 1), 24))
    );
    assert!(e.configure_interface(ep.clone(), mac, None, false).is_err());
    e.configure_interface(ep.clone(), mac, Some((ip(172, 17, 1, 1), 24)), true)
        .unwrap();
    assert!(
        e.router_routes(&r)
            .unwrap()
            .iter()
            .any(|r| r.network == Ipv4Network::new(ip(172, 17, 1, 0), 24).unwrap())
    );
    e.add_router_static_route(
        &r,
        Ipv4Network::new(ip(10, 0, 0, 0), 8).unwrap(),
        Some(ip(172, 17, 1, 2)),
        "b".into(),
        7,
    )
    .unwrap();
    let before = e.router_routes(&r).unwrap();
    assert_eq!(
        e.configure_interface(ep.clone(), mac, Some((ip(172, 18, 1, 1), 24)), false),
        Err(EngineError::Ipv4Configuration(
            ConfigurationError::NextHopNotDirectlyConnected
        ))
    );
    assert_eq!(e.router_routes(&r).unwrap(), before);
    assert!(e.interface_is_operational(&ep).unwrap());
    assert_eq!(
        e.interface_ipv4(&ep).unwrap(),
        Some((ip(172, 17, 1, 1), 24))
    );
    assert!(e.get_trace_snapshot().records.is_empty());
    assert_eq!(e.now().0, 0);
}

fn active_hosts(e: &mut Engine) {
    for (name, last) in [("h", 10), ("other", 20)] {
        e.create_host(name.into(), vec!["eth0".into()]).unwrap();
        e.configure_interface(
            EndpointRef::new(name, "eth0"),
            MacAddress([2, 0, 0, 0, 0, last]),
            Some((ip(192, 168, 5, last), 24)),
            true,
        )
        .unwrap();
    }
    e.connect_link(
        "traffic".into(),
        EndpointRef::new("h", "eth0"),
        EndpointRef::new("other", "eth0"),
    )
    .unwrap();
    e.send_ping("h".into(), ip(192, 168, 5, 20), 1, 1).unwrap();
}

#[test]
fn router_and_route_mutations_are_busy_atomic_but_admin_and_links_are_allowed() {
    let mut e = engine();
    active_hosts(&mut e);
    let before = e.get_trace_snapshot();
    let routes = e.router_routes(&"r".into()).unwrap();
    assert_eq!(
        e.add_router_static_route(
            &"r".into(),
            Ipv4Network::new(ip(10, 0, 0, 0), 8).unwrap(),
            None,
            "a".into(),
            1
        ),
        Err(EngineError::EngineBusy)
    );
    assert_eq!(
        e.configure_interface(
            EndpointRef::new("r", "a"),
            MacAddress([2, 0, 0, 0, 0, 99]),
            Some((ip(192, 168, 2, 1), 24)),
            false
        ),
        Err(EngineError::EngineBusy)
    );
    assert_eq!(
        e.create_router("new".into(), ports()),
        Err(EngineError::EngineBusy)
    );
    assert_eq!(e.router_routes(&"r".into()).unwrap(), routes);
    assert_eq!(
        e.interface_ipv4(&EndpointRef::new("r", "a")).unwrap(),
        Some((ip(192, 168, 1, 1), 24))
    );
    assert_eq!(e.get_trace_snapshot(), before);
    assert_eq!(e.now().0, 0);
    e.set_interface_admin_state(EndpointRef::new("r", "a"), false)
        .unwrap();
    e.create_host("spare".into(), vec!["p".into()]).unwrap();
    e.connect_link(
        "sparelink".into(),
        EndpointRef::new("r", "a"),
        EndpointRef::new("spare", "p"),
    )
    .unwrap();
    e.disconnect_link(&"sparelink".into()).unwrap();
    assert_eq!(
        e.run_until_quiescent(100).unwrap().outcome,
        ExecutionOutcome::Quiescent
    );
    let after = e.get_trace_snapshot();
    let time = e.now();
    e.add_router_static_route(
        &"r".into(),
        Ipv4Network::new(ip(10, 0, 0, 0), 8).unwrap(),
        None,
        "a".into(),
        1,
    )
    .unwrap();
    e.configure_interface(
        EndpointRef::new("r", "b"),
        MacAddress([2, 0, 0, 0, 0, 2]),
        Some((ip(172, 17, 1, 1), 24)),
        true,
    )
    .unwrap();
    e.create_router("new".into(), ports()).unwrap();
    assert_eq!(e.get_trace_snapshot(), after);
    assert_eq!(e.now(), time);
}

#[test]
fn registered_router_uses_scoped_arp_without_l2_switch_learning() {
    let mut e = engine();
    e.create_host("h".into(), vec!["p".into()]).unwrap();
    e.configure_interface(
        EndpointRef::new("h", "p"),
        MacAddress([2, 0, 0, 0, 0, 10]),
        Some((ip(192, 168, 1, 10), 24)),
        true,
    )
    .unwrap();
    e.connect_link(
        "wire".into(),
        EndpointRef::new("h", "p"),
        EndpointRef::new("r", "a"),
    )
    .unwrap();
    e.send_ping("h".into(), ip(192, 168, 1, 1), 1, 1).unwrap();
    assert_eq!(
        e.run_until_quiescent(100).unwrap().outcome,
        ExecutionOutcome::Quiescent
    );
    assert_eq!(e.mac_table_len(&"r".into()), 0);
    assert_eq!(e.arp_entry(&"r".into(), ip(192, 168, 1, 10)), None);
    assert_eq!(e.router_arp_entry(&EndpointRef::new("r","a"),ip(192,168,1,10)).unwrap(),Some(MacAddress([2,0,0,0,0,10])));
}

#[test]
fn router_uses_existing_link_guards_and_invalid_inputs_preserve_configuration() {
    let mut e = engine();
    let r = "r".into();
    let before = e.router_routes(&r).unwrap();
    assert!(
        e.connect_link(
            "self".into(),
            EndpointRef::new("r", "a"),
            EndpointRef::new("r", "a")
        )
        .is_err()
    );
    assert!(
        e.connect_link(
            "unknown".into(),
            EndpointRef::new("r", "a"),
            EndpointRef::new("r", "missing")
        )
        .is_err()
    );
    assert!(
        !e.interface_is_operational(&EndpointRef::new("r", "a"))
            .unwrap()
    );
    assert!(
        e.add_router_static_route(
            &r,
            Ipv4Network::new(ip(10, 0, 0, 0), 8).unwrap(),
            None,
            "absent".into(),
            0
        )
        .is_err()
    );
    assert!(
        e.configure_interface(
            EndpointRef::new("r", "absent"),
            MacAddress([2, 0, 0, 0, 0, 1]),
            Some((ip(10, 0, 0, 1), 24)),
            true
        )
        .is_err()
    );
    assert_eq!(e.router_routes(&r).unwrap(), before);
    let mut connected_duplicate = ports();
    connected_duplicate[1].ipv4 = Ipv4InterfaceConfig::new(ip(192, 168, 1, 2), 24).unwrap();
    connected_duplicate[1].admin_up = false;
    assert_eq!(
        e.create_router("invalid".into(), connected_duplicate),
        Err(EngineError::Ipv4Configuration(
            ConfigurationError::DuplicateConnectedPrefix
        ))
    );
    e.create_host("invalid".into(), vec!["p".into()]).unwrap();
    assert!(e.create_router("invalid".into(), ports()).is_err());
    assert!(e.router_routes(&"invalid".into()).is_err());
    assert!(e.router_routes(&"absent".into()).is_err());
    assert!(
        e.lookup_router_route(&"invalid".into(), ip(10, 0, 0, 1))
            .is_err()
    );
    e.connect_link(
        "good".into(),
        EndpointRef::new("r", "a"),
        EndpointRef::new("invalid", "p"),
    )
    .unwrap();
    assert!(
        e.interface_is_operational(&EndpointRef::new("r", "a"))
            .unwrap()
    );
    assert!(
        e.connect_link(
            "another".into(),
            EndpointRef::new("r", "a"),
            EndpointRef::new("r", "b")
        )
        .is_err()
    );
    assert!(
        e.connect_link(
            "good".into(),
            EndpointRef::new("r", "b"),
            EndpointRef::new("invalid", "p")
        )
        .is_err()
    );
    let mut detached = e.router_routes(&r).unwrap();
    detached.clear();
    assert_eq!(e.router_routes(&r).unwrap(), before);
    assert!(e.get_trace_snapshot().records.is_empty());
}
