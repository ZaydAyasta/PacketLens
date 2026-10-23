//! Real DES observations for differential tests, not an E1 canonical report.
use spike_e1_rust::ipv4::{HostIpv4Config, Ipv4InterfaceConfig};
use spike_e1_rust::router_engine::RouterPortConfig;
use spike_e1_rust::*;

fn ip(a: u8, b: u8, c: u8, d: u8) -> Ipv4Address {
    Ipv4Address([a, b, c, d])
}
fn addr(a: Ipv4Address) -> String {
    format!("{}.{}.{}.{}", a.0[0], a.0[1], a.0[2], a.0[3])
}
fn mac(a: MacAddress) -> String {
    a.0.iter()
        .map(|b| format!("{b:02x}"))
        .collect::<Vec<_>>()
        .join(":")
}
fn endpoint(a: &EndpointRef) -> String {
    format!("{}:{}", a.device.0, a.interface.0)
}
fn packet(p: &Ipv4Packet) -> String {
    let Ipv4Payload::Icmp(c) = &p.payload else { panic!("S1 contains only Echo"); };
    format!(
        "ip,{},{},{},{:?},{},{}",
        addr(p.src_ip),
        addr(p.dst_ip),
        p.ttl,
        c.kind,
        c.identifier,
        c.sequence
    )
}
fn frame(f: &EthernetFrame) -> String {
    let pdu = match &f.payload {
        EthernetPayload::Ipv4(p) => packet(p),
        EthernetPayload::Arp(a) => format!(
            "arp,{:?},{},{},{},{}",
            a.operation,
            addr(a.sender_ip),
            mac(a.sender_mac),
            addr(a.target_ip),
            mac(a.target_mac)
        ),
    };
    format!("eth,{},{},{pdu}", mac(f.src_mac), mac(f.dst_mac))
}
fn fixture() -> Engine {
    let mut e = Engine::with_default_trace_capacity();
    fn add_host(
        e: &mut Engine,
        name: &str,
        address: Ipv4Address,
        gateway: Ipv4Address,
        m: MacAddress,
    ) {
        e.create_host(name.into(), vec!["eth0".into()]).unwrap();
        e.configure_host_ipv4(
            EndpointRef::new(name, "eth0"),
            m,
            HostIpv4Config::new(
                Ipv4InterfaceConfig::new(address, 24).unwrap(),
                Some(gateway),
            )
            .unwrap(),
            true,
        )
        .unwrap();
    }
    fn add_switch(e: &mut Engine, sw: &str, d: u8) {
        e.create_l2_switch(sw.into(), vec!["port1".into(), "port2".into()])
            .unwrap();
        for p in 1..=2 {
            e.configure_interface(
                EndpointRef::new(sw, format!("port{p}").as_str()),
                MacAddress([2, 0, 0, 0, d, p]),
                None,
                true,
            )
            .unwrap();
        }
    }
    add_host(
        &mut e,
        "host1",
        ip(192, 168, 1, 10),
        ip(192, 168, 1, 1),
        MacAddress([0, 0, 0, 0, 1, 1]),
    );
    add_switch(&mut e, "sw1", 2);
    e.create_router(
        "r1".into(),
        vec![
            RouterPortConfig::new(
                "eth0".into(),
                MacAddress([0, 0, 0, 0, 1, 254]),
                Ipv4InterfaceConfig::new(ip(192, 168, 1, 1), 24).unwrap(),
                true,
            ),
            RouterPortConfig::new(
                "eth1".into(),
                MacAddress([0, 0, 0, 0, 2, 254]),
                Ipv4InterfaceConfig::new(ip(10, 0, 0, 1), 24).unwrap(),
                true,
            ),
        ],
    )
    .unwrap();
    add_switch(&mut e, "sw2", 4);
    add_host(
        &mut e,
        "host2",
        ip(10, 0, 0, 20),
        ip(10, 0, 0, 1),
        MacAddress([0, 0, 0, 0, 2, 2]),
    );
    for (link, a, pa, b, pb) in [
        ("l1", "host1", "eth0", "sw1", "port1"),
        ("l2", "sw1", "port2", "r1", "eth0"),
        ("l3", "r1", "eth1", "sw2", "port1"),
        ("l4", "sw2", "port2", "host2", "eth0"),
    ] {
        e.connect_link(
            link.into(),
            EndpointRef::new(a, pa),
            EndpointRef::new(b, pb),
        )
        .unwrap();
    }
    e
}
fn project() -> Vec<String> {
    let mut rows = vec![];
    for scenario in ["cold", "warm-reverse", "concurrent"] {
        let mut e = fixture();
        let ids: Vec<u16> = if scenario == "concurrent" {
            vec![3, 1, 2]
        } else {
            vec![1]
        };
        for id in ids {
            e.send_ping("host1".into(), ip(10, 0, 0, 20), id, 1)
                .unwrap();
        }
        let mut outcome = e.run_until_quiescent(1000).unwrap().outcome;
        if scenario == "warm-reverse" {
            e.send_ping("host2".into(), ip(192, 168, 1, 10), 7, 1)
                .unwrap();
            outcome = e.run_until_quiescent(1000).unwrap().outcome;
        }
        let trace = e.get_trace_snapshot();
        rows.push(format!(
            "{scenario}|result|{outcome:?}|{}|{}|{}",
            e.now().0,
            trace.total_emitted_ever,
            trace.dropped_by_capacity
        ));
        for r in trace.records {
            let detail = match &r.event {
                ObservableEvent::FrameTransmitted {
                    link,
                    from,
                    to,
                    arrives_at,
                    frame: f,
                } => format!(
                    "tx|{}|{}|{}|{}|{}",
                    link.0,
                    endpoint(from),
                    endpoint(to),
                    arrives_at.0,
                    frame(f)
                ),
                ObservableEvent::FrameDelivered {
                    link,
                    at_endpoint,
                    frame: f,
                } => format!("rx|{}|{}|{}", link.0, endpoint(at_endpoint), frame(f)),
                ObservableEvent::ArpPacketQueued {
                    at_device,
                    target_ip,
                    packet: p,
                } => format!("queued|{}|{}|{}", at_device.0, addr(*target_ip), packet(p)),
                ObservableEvent::TableUpdated { at_device, update } => match update {
                    TableUpdateDetail::ArpLearned {
                        ip,
                        mac: m,
                        expires_at,
                    } => format!(
                        "arp|{}|{}|{}|{}",
                        at_device.0,
                        addr(*ip),
                        mac(*m),
                        expires_at.0
                    ),
                    TableUpdateDetail::MacLearned {
                        mac: m,
                        port,
                        expires_at,
                    } => format!(
                        "cam|{}|{}|{}|{}",
                        at_device.0,
                        mac(*m),
                        port.0,
                        expires_at.0
                    ),
                    other => panic!("Unexpected expiry in short scenario: {other:?}"),
                },
                ObservableEvent::PingCompleted {
                    host,
                    target_ip,
                    identifier,
                    sequence,
                    rtt_ns,
                } => format!(
                    "completed|{}|{}|{}|{}|{}",
                    host.0,
                    addr(*target_ip),
                    identifier,
                    sequence,
                    rtt_ns
                ),
                other => panic!("Unexpected non-success observation: {other:?}"),
            };
            rows.push(format!(
                "{scenario}|{}|{}|{}|{detail}",
                r.seq.0,
                r.at.0,
                r.observation_interface
                    .as_ref()
                    .map(|n| n.0.as_str())
                    .unwrap_or("-")
            ));
        }
        for (host, target) in [("host1", ip(192, 168, 1, 1)), ("host2", ip(10, 0, 0, 1))] {
            rows.push(format!(
                "{scenario}|host-arp|{host}|{}|{}",
                addr(target),
                mac(e.arp_entry(&host.into(), target).unwrap())
            ));
        }
        for (port, target) in [("eth0", ip(192, 168, 1, 10)), ("eth1", ip(10, 0, 0, 20))] {
            rows.push(format!(
                "{scenario}|router-arp|{port}|{}|{}",
                addr(target),
                mac(e
                    .router_arp_entry(&EndpointRef::new("r1", port), target)
                    .unwrap()
                    .unwrap())
            ));
        }
        for (sw, m) in [
            ("sw1", MacAddress([0, 0, 0, 0, 1, 1])),
            ("sw1", MacAddress([0, 0, 0, 0, 1, 254])),
            ("sw2", MacAddress([0, 0, 0, 0, 2, 254])),
            ("sw2", MacAddress([0, 0, 0, 0, 2, 2])),
        ] {
            rows.push(format!(
                "{scenario}|cam-final|{sw}|{}|{}|{}",
                mac(m),
                e.mac_entry(&sw.into(), m).unwrap().0,
                e.mac_table_len(&sw.into())
            ));
        }
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
