use spike_e1_rust::ipv4::{HostIpv4Config, Ipv4InterfaceConfig};
use spike_e1_rust::{
    DeviceRef, EndpointRef, Engine, EngineError, InterfaceName, Ipv4Address, MacAddress,
};

fn settings(last: u8, gateway: Option<u8>) -> HostIpv4Config {
    HostIpv4Config::new(
        Ipv4InterfaceConfig::new(Ipv4Address([192, 168, 1, last]), 24).unwrap(),
        gateway.map(|n| Ipv4Address([192, 168, 1, n])),
    )
    .unwrap()
}

#[test]
fn cfg_01_engine_host_configuration_does_not_emit_or_advance() {
    let mut engine = Engine::with_default_trace_capacity();
    engine
        .create_host(DeviceRef::from("host"), vec![InterfaceName::from("eth0")])
        .unwrap();
    let before = engine.get_trace_snapshot();
    engine
        .configure_host_ipv4(
            EndpointRef::new("host", "eth0"),
            MacAddress([2, 0, 0, 0, 0, 1]),
            settings(10, Some(1)),
            true,
        )
        .unwrap();
    assert_eq!(
        engine.host_ipv4_config(&DeviceRef::from("host")),
        Some(settings(10, Some(1)))
    );
    assert_eq!(engine.get_trace_snapshot(), before);
    assert_eq!(engine.now().0, 0);
}

#[test]
fn cfg_01_invalid_legacy_update_preserves_gateway_and_interface() {
    let mut engine = Engine::with_default_trace_capacity();
    engine
        .create_host("host".into(), vec!["eth0".into()])
        .unwrap();
    let endpoint = EndpointRef::new("host", "eth0");
    let mac = MacAddress([2, 0, 0, 0, 0, 1]);
    engine
        .configure_host_ipv4(endpoint.clone(), mac, settings(10, Some(1)), true)
        .unwrap();
    let before = engine.get_trace_snapshot();
    assert_eq!(
        engine.configure_interface(
            endpoint.clone(),
            mac,
            Some((Ipv4Address([192, 168, 2, 10]), 24)),
            true
        ),
        Err(EngineError::Ipv4Configuration(
            spike_e1_rust::ipv4::ConfigurationError::InvalidGateway
        ))
    );
    assert!(
        engine
            .configure_interface(endpoint, mac, None, true)
            .is_err()
    );
    assert_eq!(
        engine.host_ipv4_config(&"host".into()),
        Some(settings(10, Some(1)))
    );
    assert_eq!(engine.get_trace_snapshot(), before);
    assert_eq!(engine.now().0, 0);
}

#[test]
fn cfg_01_e2_configuration_rejects_nonhost_and_ambiguous_host() {
    let mut engine = Engine::with_default_trace_capacity();
    engine
        .create_l2_switch("switch".into(), vec!["p1".into()])
        .unwrap();
    engine
        .create_host("host".into(), vec!["eth0".into(), "eth1".into()])
        .unwrap();
    let mac = MacAddress([2, 0, 0, 0, 0, 1]);
    engine
        .configure_interface(
            EndpointRef::new("host", "eth1"),
            mac,
            Some((Ipv4Address([192, 168, 1, 20]), 24)),
            true,
        )
        .unwrap();
    for endpoint in [
        EndpointRef::new("switch", "p1"),
        EndpointRef::new("host", "eth0"),
        EndpointRef::new("host", "missing"),
    ] {
        assert!(
            engine
                .configure_host_ipv4(endpoint, mac, settings(10, None), true)
                .is_err()
        );
    }
    assert_eq!(engine.host_ipv4_config(&"host".into()), None);
    assert!(engine.get_trace_snapshot().records.is_empty());
}

#[test]
fn cfg_02_busy_guard_cannot_be_bypassed_by_legacy_interface_configuration() {
    let mut engine = Engine::with_default_trace_capacity();
    for host in ["host", "peer"] {
        engine
            .create_host(DeviceRef::from(host), vec![InterfaceName::from("eth0")])
            .unwrap();
    }
    engine
        .configure_host_ipv4(
            EndpointRef::new("host", "eth0"),
            MacAddress([2, 0, 0, 0, 0, 1]),
            settings(10, None),
            true,
        )
        .unwrap();
    engine
        .configure_interface(
            EndpointRef::new("peer", "eth0"),
            MacAddress([2, 0, 0, 0, 0, 2]),
            Some((Ipv4Address([192, 168, 1, 20]), 24)),
            true,
        )
        .unwrap();
    engine
        .connect_link(
            "link".into(),
            EndpointRef::new("host", "eth0"),
            EndpointRef::new("peer", "eth0"),
        )
        .unwrap();
    engine
        .send_ping("host".into(), Ipv4Address([192, 168, 1, 20]), 1, 1)
        .unwrap();
    let before = engine.get_trace_snapshot();
    assert_eq!(
        engine.configure_host_ipv4(
            EndpointRef::new("host", "eth0"),
            MacAddress([2, 0, 0, 0, 0, 1]),
            settings(11, None),
            true
        ),
        Err(EngineError::EngineBusy)
    );
    assert_eq!(
        engine.configure_interface(
            EndpointRef::new("host", "eth0"),
            MacAddress([2, 0, 0, 0, 0, 1]),
            Some((Ipv4Address([192, 168, 1, 11]), 24)),
            true
        ),
        Err(EngineError::EngineBusy)
    );
    assert_eq!(
        engine.host_ipv4_config(&"host".into()),
        Some(settings(10, None))
    );
    assert_eq!(engine.get_trace_snapshot(), before);
    assert_eq!(engine.now().0, 0);
    engine.run_until_quiescent(100).unwrap();
    let after_run = engine.get_trace_snapshot();
    engine
        .configure_host_ipv4(
            EndpointRef::new("host", "eth0"),
            MacAddress([2, 0, 0, 0, 0, 1]),
            settings(11, None),
            true,
        )
        .unwrap();
    assert_eq!(
        engine.host_ipv4_config(&"host".into()),
        Some(settings(11, None))
    );
    assert_eq!(engine.get_trace_snapshot(), after_run);
}
