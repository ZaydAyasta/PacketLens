//! Independent diagnostic projection; not a public DTO or a golden.
use spike_e1_rust::ipv4::{Ipv4InterfaceConfig, Ipv4LinkMode, Ipv4Network};
use spike_e1_rust::router_engine::RouterPortConfig;
use spike_e1_rust::*;

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
fn error(value: Result<(), EngineError>) -> String {
    match value {
        Ok(()) => "ok".into(),
        Err(EngineError::Ipv4Configuration(c)) => format!("{c:?}"),
        Err(e) => format!("{e:?}"),
    }
}
fn state(e: &Engine, label: &str) -> String {
    let select = |target| {
        e.lookup_router_route(&"r".into(), target)
            .unwrap()
            .map(|r| format!("{}/{}", r.egress_interface.0, r.network.prefix().value()))
            .unwrap_or_else(|| "none".into())
    };
    format!(
        "state|{label}|{}|{}|{}|{}|{}",
        u8::from(
            e.interface_is_operational(&EndpointRef::new("r", "a"))
                .unwrap()
        ),
        u8::from(
            e.interface_is_operational(&EndpointRef::new("r", "b"))
                .unwrap()
        ),
        select(ip(10, 1, 1, 1)),
        select(ip(192, 168, 1, 10)),
        e.router_routes(&"r".into()).unwrap().len()
    )
}
fn pair(last: u8, prefix: u8, mode: bool, kind: u8) -> Engine {
    let mut e = Engine::with_default_trace_capacity();
    for (host, address, length, declared) in [("h0", 0, 31, true), ("h1", last, prefix, mode)] {
        e.create_host(host.into(), vec!["p".into()]).unwrap();
        if declared {
            e.set_interface_ipv4_mode(EndpointRef::new(host, "p"), Ipv4LinkMode::PointToPoint)
                .unwrap();
        }
        e.configure_interface(
            EndpointRef::new(host, "p"),
            MacAddress([2, 0, 0, 0, 0, address + 1]),
            Some((ip(172, 16, 0, address), length)),
            true,
        )
        .unwrap();
    }
    if kind == 0 {
        e.connect_link(
            "wire".into(),
            EndpointRef::new("h0", "p"),
            EndpointRef::new("h1", "p"),
        )
        .unwrap();
    }
    if kind == 2 {
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
    }
    e
}
fn project() -> Vec<String> {
    let mut rows = vec![];
    let mut e = Engine::with_default_trace_capacity();
    e.create_router("r".into(), ports()).unwrap();
    e.create_host("peer".into(), vec!["x".into(), "y".into()])
        .unwrap();
    e.add_router_static_route(
        &"r".into(),
        Ipv4Network::new(ip(10, 0, 0, 0), 8).unwrap(),
        Some(ip(172, 16, 1, 2)),
        "b".into(),
        1,
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
    rows.push(state(&e, "registered"));
    e.connect_link(
        "left".into(),
        EndpointRef::new("r", "a"),
        EndpointRef::new("peer", "x"),
    )
    .unwrap();
    rows.push(state(&e, "left"));
    e.connect_link(
        "right".into(),
        EndpointRef::new("r", "b"),
        EndpointRef::new("peer", "y"),
    )
    .unwrap();
    rows.push(state(&e, "right"));
    e.set_interface_admin_state(EndpointRef::new("r", "b"), false)
        .unwrap();
    rows.push(state(&e, "down"));
    e.set_interface_admin_state(EndpointRef::new("r", "b"), true)
        .unwrap();
    rows.push(state(&e, "up"));
    e.set_interface_admin_state(EndpointRef::new("peer", "y"), false)
        .unwrap();
    rows.push(state(&e, "peer-down"));
    e.disconnect_link(&"right".into()).unwrap();
    rows.push(state(&e, "disconnected"));
    e.connect_link(
        "right".into(),
        EndpointRef::new("r", "b"),
        EndpointRef::new("peer", "y"),
    )
    .unwrap();
    rows.push(state(&e, "reconnected"));
    rows.push(format!(
        "reject|duplicate|{}",
        error(e.add_router_static_route(
            &"r".into(),
            Ipv4Network::new(ip(10, 1, 1, 1), 8).unwrap(),
            None,
            "a".into(),
            99
        ))
    ));
    rows.push(format!(
        "reject|connected|{}",
        error(e.configure_interface(
            EndpointRef::new("r", "b"),
            MacAddress([2, 0, 0, 0, 0, 2]),
            Some((ip(192, 168, 1, 2), 24)),
            false
        ))
    ));
    rows.push(format!(
        "reject|next-hop|{}",
        error(e.configure_interface(
            EndpointRef::new("r", "b"),
            MacAddress([2, 0, 0, 0, 0, 2]),
            Some((ip(172, 17, 1, 1), 24)),
            false
        ))
    ));
    rows.push(state(&e, "after-rejections"));
    for (host, last) in [("tx", 10), ("rx", 20)] {
        e.create_host(host.into(), vec!["p".into()]).unwrap();
        e.configure_interface(
            EndpointRef::new(host, "p"),
            MacAddress([2, 0, 0, 0, 1, last]),
            Some((ip(192, 168, 5, last), 24)),
            true,
        )
        .unwrap();
    }
    e.connect_link(
        "traffic".into(),
        EndpointRef::new("tx", "p"),
        EndpointRef::new("rx", "p"),
    )
    .unwrap();
    e.send_ping("tx".into(), ip(192, 168, 5, 20), 1, 1).unwrap();
    rows.push(format!(
        "busy|route|{}",
        error(e.add_router_static_route(
            &"r".into(),
            Ipv4Network::new(ip(11, 0, 0, 0), 8).unwrap(),
            None,
            "a".into(),
            0
        ))
    ));
    rows.push(format!(
        "busy|port|{}",
        error(e.configure_interface(
            EndpointRef::new("r", "b"),
            MacAddress([2, 0, 0, 0, 0, 2]),
            Some((ip(172, 17, 1, 1), 24)),
            false
        ))
    ));
    rows.push(format!(
        "busy|registration|{}",
        error(e.create_router("new".into(), ports()))
    ));
    rows.push(state(&e, "after-busy"));
    e.run_until_quiescent(100).unwrap();
    rows.push(format!(
        "idle|{}",
        error(e.add_router_static_route(
            &"r".into(),
            Ipv4Network::new(ip(11, 0, 0, 0), 8).unwrap(),
            None,
            "a".into(),
            0
        ))
    ));
    for (label, last, prefix, mode, kind) in [
        ("valid", 1, 31, true, 0),
        ("same-ip", 0, 31, true, 0),
        ("wrong-pair", 2, 31, true, 0),
        ("wrong-prefix", 1, 30, true, 0),
        ("no-mode", 1, 31, false, 0),
        ("unwired", 1, 31, true, 1),
        ("switch", 1, 31, true, 2),
    ] {
        rows.push(format!(
            "topology|{label}|{}",
            error(pair(last, prefix, mode, kind).validate_ipv4_topology())
        ));
    }
    let mut pair = pair(1, 31, true, 0);
    for (source, last, id) in [("h0", 1, 1), ("h1", 0, 2)] {
        pair.send_ping(source.into(), ip(172, 16, 0, last), id, 1)
            .unwrap();
        pair.run_until_quiescent(100).unwrap();
        let count = pair
            .get_trace_snapshot()
            .records
            .iter()
            .filter(
                |r| matches!(&r.event,ObservableEvent::PingCompleted { host,.. } if host.0==source),
            )
            .count();
        rows.push(format!("direct-ping|{source}|{count}"));
    }
    rows
}
fn main() {
    let expected = project();
    for _ in 1..100 {
        assert_eq!(project(), expected);
    }
    for row in expected {
        println!("{row}");
    }
}
