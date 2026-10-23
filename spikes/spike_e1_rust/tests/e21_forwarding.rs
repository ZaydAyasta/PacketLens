use spike_e1_rust::ipv4::{HostIpv4Config, Ipv4InterfaceConfig};
use spike_e1_rust::router_engine::RouterPortConfig;
use spike_e1_rust::*;

fn router(e: &mut Engine) {
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
}
fn s1_fixture() -> Engine {
    let mut e = Engine::with_default_trace_capacity();
    host(
        &mut e,
        "host1",
        ip(192, 168, 1, 10),
        Some(ip(192, 168, 1, 1)),
        MacAddress([0, 0, 0, 0, 1, 1]),
    );
    e.create_l2_switch("sw1".into(), vec!["port1".into(), "port2".into()])
        .unwrap();
    router(&mut e);
    e.create_l2_switch("sw2".into(), vec!["port1".into(), "port2".into()])
        .unwrap();
    host(
        &mut e,
        "host2",
        ip(10, 0, 0, 20),
        Some(ip(10, 0, 0, 1)),
        MacAddress([0, 0, 0, 0, 2, 2]),
    );
    for (sw, d) in [("sw1", 2), ("sw2", 4)] {
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

fn ip(a: u8, b: u8, c: u8, d: u8) -> Ipv4Address {
    Ipv4Address([a, b, c, d])
}
fn host(
    e: &mut Engine,
    name: &str,
    address: Ipv4Address,
    gateway: Option<Ipv4Address>,
    mac: MacAddress,
) {
    e.create_host(name.into(), vec!["eth0".into()]).unwrap();
    e.configure_host_ipv4(
        EndpointRef::new(name, "eth0"),
        mac,
        HostIpv4Config::new(Ipv4InterfaceConfig::new(address, 24).unwrap(), gateway).unwrap(),
        true,
    )
    .unwrap();
}

#[test]
fn host_remote_ping_arps_only_gateway_and_queues_original_ip_datagram() {
    let mut e = Engine::with_default_trace_capacity();
    host(
        &mut e,
        "h",
        ip(192, 168, 1, 10),
        Some(ip(192, 168, 1, 1)),
        MacAddress([0, 0, 0, 0, 1, 1]),
    );
    e.create_l2_switch("sw".into(), vec!["p".into()]).unwrap();
    e.connect_link(
        "wire".into(),
        EndpointRef::new("h", "eth0"),
        EndpointRef::new("sw", "p"),
    )
    .unwrap();
    e.send_ping("h".into(), ip(10, 0, 0, 20), 1, 1).unwrap();
    e.step_events(1).unwrap();
    let trace = e.get_trace_snapshot();
    assert!(trace.records.iter().any(|r| matches!(&r.event,ObservableEvent::ArpPacketQueued { target_ip,packet,.. } if *target_ip==ip(192,168,1,1) && packet.src_ip==ip(192,168,1,10) && packet.dst_ip==ip(10,0,0,20) && packet.ttl==64)));
    assert!(trace.records.iter().any(|r| matches!(&r.event,ObservableEvent::FrameTransmitted { frame: EthernetFrame { payload: EthernetPayload::Arp(a),.. },.. } if a.target_ip==ip(192,168,1,1) && a.operation==ArpOperation::Request)));
    assert_eq!(e.now().0, 0);
}

#[test]
fn router_answers_only_ingress_arp_and_learns_a_scoped_entry() {
    let mut e = Engine::with_default_trace_capacity();
    host(
        &mut e,
        "h",
        ip(192, 168, 1, 10),
        None,
        MacAddress([0, 0, 0, 0, 1, 1]),
    );
    router(&mut e);
    e.connect_link(
        "wire".into(),
        EndpointRef::new("h", "eth0"),
        EndpointRef::new("r1", "eth0"),
    )
    .unwrap();
    e.send_ping("h".into(), ip(192, 168, 1, 1), 1, 1).unwrap();
    e.advance_until(VirtualTime(2200), 100).unwrap();
    assert_eq!(
        e.arp_entry(&"h".into(), ip(192, 168, 1, 1)),
        Some(MacAddress([0, 0, 0, 0, 1, 254]))
    );
    assert_eq!(
        e.router_arp_entry(&EndpointRef::new("r1", "eth0"), ip(192, 168, 1, 10))
            .unwrap(),
        Some(MacAddress([0, 0, 0, 0, 1, 1]))
    );
    assert_eq!(
        e.router_arp_entry(&EndpointRef::new("r1", "eth1"), ip(192, 168, 1, 10))
            .unwrap(),
        None
    );
    let trace = e.get_trace_snapshot();
    assert!(trace.records.iter().any(|r| r.observation_interface==Some("eth0".into()) && matches!(&r.event,ObservableEvent::TableUpdated { at_device,update:TableUpdateDetail::ArpLearned { ip: learned,.. } } if at_device.0=="r1" && *learned==ip(192,168,1,10))));
    assert!(!trace.records.iter().any(|r| matches!(&r.event,ObservableEvent::FrameTransmitted { from,.. } if from.device.0=="r1" && from.interface.0=="eth1")));
}

#[test]
fn e21_s1_cold_round_trip_preserves_ip_decrements_ttl_once_and_matches_exact_tables() {
    let mut e = s1_fixture();
    e.send_ping("host1".into(), ip(10, 0, 0, 20), 1, 1).unwrap();
    assert_eq!(
        e.run_until_quiescent(1000).unwrap().outcome,
        ExecutionOutcome::Quiescent
    );
    let trace = e.get_trace_snapshot();
    let completed: Vec<_> = trace
        .records
        .iter()
        .filter(|r| matches!(&r.event, ObservableEvent::PingCompleted { .. }))
        .collect();
    assert_eq!(completed.len(), 1);
    assert!(
        matches!(&completed[0].event,ObservableEvent::PingCompleted { host,rtt_ns:17500,identifier:1,sequence:1,.. } if host.0=="host1")
    );
    assert_eq!(e.now().0, 17500);
    assert!(!trace.records.iter().any(|r| matches!(
        &r.event,
        ObservableEvent::PingFailed { .. } | ObservableEvent::PacketDropped { .. }
    )));
    assert_eq!(
        trace
            .records
            .iter()
            .filter(|r| matches!(
                &r.event,
                ObservableEvent::TableUpdated {
                    update: TableUpdateDetail::ArpLearned { .. },
                    ..
                }
            ))
            .count(),
        4
    );
    for (device, port, source, dest, ttl, smac, dmac) in [
        (
            "host1",
            "eth0",
            ip(192, 168, 1, 10),
            ip(10, 0, 0, 20),
            64,
            MacAddress([0, 0, 0, 0, 1, 1]),
            MacAddress([0, 0, 0, 0, 1, 254]),
        ),
        (
            "r1",
            "eth1",
            ip(192, 168, 1, 10),
            ip(10, 0, 0, 20),
            63,
            MacAddress([0, 0, 0, 0, 2, 254]),
            MacAddress([0, 0, 0, 0, 2, 2]),
        ),
        (
            "host2",
            "eth0",
            ip(10, 0, 0, 20),
            ip(192, 168, 1, 10),
            64,
            MacAddress([0, 0, 0, 0, 2, 2]),
            MacAddress([0, 0, 0, 0, 2, 254]),
        ),
        (
            "r1",
            "eth0",
            ip(10, 0, 0, 20),
            ip(192, 168, 1, 10),
            63,
            MacAddress([0, 0, 0, 0, 1, 254]),
            MacAddress([0, 0, 0, 0, 1, 1]),
        ),
    ] {
        let frames: Vec<_> = trace
            .records
            .iter()
            .filter_map(|r| match &r.event {
                ObservableEvent::FrameTransmitted { from, frame, .. }
                    if from == &EndpointRef::new(device, port)
                        && matches!(frame.payload, EthernetPayload::Ipv4(_)) =>
                {
                    Some(frame)
                }
                _ => None,
            })
            .collect();
        assert_eq!(frames.len(), 1);
        assert_eq!((frames[0].src_mac, frames[0].dst_mac), (smac, dmac));
        let EthernetPayload::Ipv4(packet) = &frames[0].payload else {
            panic!("IPv4 expected");
        };
        assert_eq!(
            (packet.src_ip, packet.dst_ip, packet.ttl),
            (source, dest, ttl)
        );
        let Ipv4Payload::Icmp(icmp) = &packet.payload else { panic!("Echo expected"); };
        assert_eq!((icmp.identifier, icmp.sequence), (1, 1));
        assert_eq!(
            icmp.kind,
            if source == ip(192, 168, 1, 10) {
                IcmpKind::EchoRequest
            } else {
                IcmpKind::EchoReply
            }
        );
    }
    for (host, target, mac) in [
        (
            "host1",
            ip(192, 168, 1, 1),
            MacAddress([0, 0, 0, 0, 1, 254]),
        ),
        ("host2", ip(10, 0, 0, 1), MacAddress([0, 0, 0, 0, 2, 254])),
    ] {
        assert_eq!(e.arp_entry(&host.into(), target), Some(mac));
    }
    assert_eq!(e.arp_entry(&"host1".into(), ip(10, 0, 0, 20)), None);
    assert_eq!(e.arp_entry(&"host2".into(), ip(192, 168, 1, 10)), None);
    assert_eq!(
        e.router_arp_entry(&EndpointRef::new("r1", "eth0"), ip(192, 168, 1, 10))
            .unwrap(),
        Some(MacAddress([0, 0, 0, 0, 1, 1]))
    );
    assert_eq!(
        e.router_arp_entry(&EndpointRef::new("r1", "eth1"), ip(10, 0, 0, 20))
            .unwrap(),
        Some(MacAddress([0, 0, 0, 0, 2, 2]))
    );
    for (sw, mac, port) in [
        ("sw1", MacAddress([0, 0, 0, 0, 1, 1]), "port1"),
        ("sw1", MacAddress([0, 0, 0, 0, 1, 254]), "port2"),
        ("sw2", MacAddress([0, 0, 0, 0, 2, 254]), "port1"),
        ("sw2", MacAddress([0, 0, 0, 0, 2, 2]), "port2"),
    ] {
        assert_eq!(e.mac_entry(&sw.into(), mac), Some(port.into()));
        assert_eq!(e.mac_table_len(&sw.into()), 2);
    }
}

#[test]
fn concurrent_remote_pings_share_resolution_and_leave_router_fifo_with_ttl_63() {
    let mut e = s1_fixture();
    for id in [3, 1, 2] {
        e.send_ping("host1".into(), ip(10, 0, 0, 20), id, 1)
            .unwrap();
    }
    e.run_until_quiescent(1000).unwrap();
    let trace = e.get_trace_snapshot();
    let ids: Vec<_> = trace
        .records
        .iter()
        .filter_map(|r| match &r.event {
            ObservableEvent::FrameTransmitted {
                from,
                frame:
                    EthernetFrame {
                        payload: EthernetPayload::Ipv4(p),
                        ..
                    },
                ..
            } if from == &EndpointRef::new("r1", "eth1") => {
                assert_eq!(p.ttl, 63);
                let Ipv4Payload::Icmp(c) = &p.payload else { panic!("Echo expected"); };
                Some(c.identifier)
            }
            _ => None,
        })
        .collect();
    assert_eq!(ids, vec![3, 1, 2]);
    let completed: Vec<_> = trace
        .records
        .iter()
        .filter_map(|r| match &r.event {
            ObservableEvent::PingCompleted {
                host, identifier, ..
            } => {
                assert_eq!(host.0, "host1");
                Some(*identifier)
            }
            _ => None,
        })
        .collect();
    assert_eq!(completed, vec![3, 1, 2]);
    for (device, port) in [("host1", "eth0"), ("r1", "eth1")] {
        assert_eq!(trace.records.iter().filter(|r|matches!(&r.event,ObservableEvent::FrameTransmitted { from,frame:EthernetFrame { payload:EthernetPayload::Arp(a),.. },.. } if from==&EndpointRef::new(device,port) && a.operation==ArpOperation::Request)).count(),1);
    }
    assert_eq!(e.now().0, 17500);
}

#[test]
fn reverse_warm_ping_uses_cached_gateway_without_refreshing_router_arp_lifetime() {
    let mut e = s1_fixture();
    e.send_ping("host1".into(), ip(10, 0, 0, 20), 1, 1).unwrap();
    e.run_until_quiescent(1000).unwrap();
    let before = e.get_trace_snapshot().newest_available_seq.unwrap();
    e.send_ping("host2".into(), ip(192, 168, 1, 10), 7, 1)
        .unwrap();
    e.run_until_quiescent(1000).unwrap();
    let trace = e.get_trace_snapshot();
    assert!(trace.records.iter().any(|r|matches!(&r.event,ObservableEvent::PingCompleted { host,identifier:7,rtt_ns:8700,.. } if host.0=="host2")));
    assert_eq!(e.now().0, 26200);
    assert!(!trace
        .records
        .iter()
        .filter(|r| r.seq > before)
        .any(|r| matches!(
            &r.event,
            ObservableEvent::TableUpdated {
                update: TableUpdateDetail::ArpLearned { .. },
                ..
            } | ObservableEvent::FrameTransmitted {
                frame: EthernetFrame {
                    payload: EthernetPayload::Arp(_),
                    ..
                },
                ..
            }
        )));
    e.advance_until(VirtualTime(60_000_002_100), 1000).unwrap();
    assert_eq!(
        e.router_arp_entry(&EndpointRef::new("r1", "eth0"), ip(192, 168, 1, 10))
            .unwrap(),
        None
    );
    assert!(e
        .router_arp_entry(&EndpointRef::new("r1", "eth1"), ip(10, 0, 0, 20))
        .unwrap()
        .is_some());
    assert!(e.get_trace_snapshot().records.iter().any(|r|r.at==VirtualTime(60_000_002_100) && r.observation_interface==Some("eth0".into()) && matches!(&r.event,ObservableEvent::TableUpdated { at_device,update:TableUpdateDetail::ArpEntryExpired { ip:expired } } if at_device.0=="r1" && *expired==ip(192,168,1,10))));
}

#[test]
fn failed_gateway_arp_closes_original_remote_ping_not_gateway_key() {
    let mut e = Engine::with_default_trace_capacity();
    host(
        &mut e,
        "h",
        ip(192, 168, 1, 10),
        Some(ip(192, 168, 1, 1)),
        MacAddress([0, 0, 0, 0, 1, 1]),
    );
    e.create_l2_switch("sw".into(), vec!["p".into()]).unwrap();
    e.connect_link(
        "wire".into(),
        EndpointRef::new("h", "eth0"),
        EndpointRef::new("sw", "p"),
    )
    .unwrap();
    e.send_ping("h".into(), ip(10, 0, 0, 20), 4, 1).unwrap();
    e.run_until_quiescent(1000).unwrap();
    assert_eq!(e.now().0, 3_000_000_000);
    let failures: Vec<_> = e
        .get_trace_snapshot()
        .records
        .into_iter()
        .filter(|r| matches!(&r.event, ObservableEvent::PingFailed { .. }))
        .collect();
    assert_eq!(failures.len(), 1);
    assert!(
        matches!(&failures[0].event,ObservableEvent::PingFailed { host,target_ip,identifier:4,reason:PingFailureReason::ArpTimeout,.. } if host.0=="h" && *target_ip==ip(10,0,0,20))
    );
    e.configure_host_ipv4(
        EndpointRef::new("h", "eth0"),
        MacAddress([0, 0, 0, 0, 1, 1]),
        HostIpv4Config::new(
            Ipv4InterfaceConfig::new(ip(192, 168, 1, 11), 24).unwrap(),
            None,
        )
        .unwrap(),
        true,
    )
    .unwrap();
}

#[test]
fn router_local_echo_uses_requested_ip_as_reply_source_not_egress_ip() {
    for target in [ip(192, 168, 1, 1), ip(10, 0, 0, 1)] {
        let mut e = Engine::with_default_trace_capacity();
        host(
            &mut e,
            "h",
            ip(192, 168, 1, 10),
            Some(ip(192, 168, 1, 1)),
            MacAddress([0, 0, 0, 0, 1, 1]),
        );
        router(&mut e);
        e.connect_link(
            "wire".into(),
            EndpointRef::new("h", "eth0"),
            EndpointRef::new("r1", "eth0"),
        )
        .unwrap();
        e.send_ping("h".into(), target, 9, 1).unwrap();
        e.run_until_quiescent(100).unwrap();
        let trace = e.get_trace_snapshot();
        assert_eq!(trace.records.iter().filter(|r|matches!(&r.event,ObservableEvent::PingCompleted {host,rtt_ns:4300,..} if host.0=="h")).count(),1);
        let replies: Vec<_> = trace
            .records
            .iter()
            .filter_map(|r| match &r.event {
                ObservableEvent::FrameTransmitted {
                    from,
                    frame:
                        EthernetFrame {
                            payload: EthernetPayload::Ipv4(p),
                            src_mac,
                            ..
                        },
                    ..
                } if from == &EndpointRef::new("r1", "eth0") => Some((p, *src_mac)),
                _ => None,
            })
            .collect();
        assert_eq!(replies.len(), 1);
        assert_eq!(
            (replies[0].0.src_ip, replies[0].0.dst_ip, replies[0].0.ttl),
            (target, ip(192, 168, 1, 10), 64)
        );
        assert_eq!(replies[0].1, MacAddress([0, 0, 0, 0, 1, 254]));
        assert!(!trace.records.iter().any(|r|matches!(&r.event,ObservableEvent::PingCompleted {host,..} | ObservableEvent::PingFailed {host,..} if host.0=="r1")));
    }
}

#[test]
fn static_host_route_arps_explicit_next_hop_not_final_destination() {
    let mut e = s1_fixture();
    e.add_router_static_route(
        &"r1".into(),
        spike_e1_rust::ipv4::Ipv4Network::new(ip(10, 0, 0, 99), 32).unwrap(),
        Some(ip(10, 0, 0, 20)),
        "eth1".into(),
        999,
    )
    .unwrap();
    e.send_ping("host1".into(), ip(10, 0, 0, 99), 1, 1).unwrap();
    e.advance_until(VirtualTime(11000), 1000).unwrap();
    let trace = e.get_trace_snapshot();
    assert!(trace.records.iter().any(|r|matches!(&r.event,ObservableEvent::ArpPacketQueued {at_device,target_ip,packet} if at_device.0=="r1" && *target_ip==ip(10,0,0,20) && packet.dst_ip==ip(10,0,0,99) && packet.ttl==63)));
    assert!(trace.records.iter().any(|r|matches!(&r.event,ObservableEvent::FrameTransmitted {from,frame:EthernetFrame {payload:EthernetPayload::Ipv4(p),dst_mac,..},..} if from==&EndpointRef::new("r1","eth1") && p.dst_ip==ip(10,0,0,99) && p.ttl==63 && *dst_mac==MacAddress([0,0,0,0,2,2]))));
    assert!(!trace.records.iter().any(|r|matches!(&r.event,ObservableEvent::FrameTransmitted {from,frame:EthernetFrame {payload:EthernetPayload::Arp(a),..},..} if from.device.0=="r1" && a.target_ip==ip(10,0,0,99))));
}

#[test]
fn identical_neighbor_ip_on_two_interfaces_has_independent_arp_entries() {
    let mut e = Engine::with_default_trace_capacity();
    e.create_router(
        "r".into(),
        vec![
            RouterPortConfig::new(
                "a".into(),
                MacAddress([2, 0, 0, 0, 1, 254]),
                Ipv4InterfaceConfig::new(ip(192, 168, 1, 1), 24).unwrap(),
                true,
            ),
            RouterPortConfig::new(
                "b".into(),
                MacAddress([2, 0, 0, 0, 2, 254]),
                Ipv4InterfaceConfig::new(ip(192, 168, 1, 129), 25).unwrap(),
                true,
            ),
        ],
    )
    .unwrap();
    for (host, port, prefix, mac) in [
        ("h1", "a", 24, MacAddress([2, 0, 0, 0, 1, 1])),
        ("h2", "b", 25, MacAddress([2, 0, 0, 0, 2, 2])),
    ] {
        e.create_host(host.into(), vec!["p".into()]).unwrap();
        e.configure_host_ipv4(
            EndpointRef::new(host, "p"),
            mac,
            HostIpv4Config::new(
                Ipv4InterfaceConfig::new(ip(192, 168, 1, 130), prefix).unwrap(),
                None,
            )
            .unwrap(),
            true,
        )
        .unwrap();
        e.connect_link(
            host.into(),
            EndpointRef::new(host, "p"),
            EndpointRef::new("r", port),
        )
        .unwrap();
    }
    e.send_ping("h1".into(), ip(192, 168, 1, 1), 1, 1).unwrap();
    e.send_ping("h2".into(), ip(192, 168, 1, 129), 2, 1)
        .unwrap();
    e.advance_until(VirtualTime(2200), 100).unwrap();
    assert_eq!(
        e.router_arp_entry(&EndpointRef::new("r", "a"), ip(192, 168, 1, 130))
            .unwrap(),
        Some(MacAddress([2, 0, 0, 0, 1, 1]))
    );
    assert_eq!(
        e.router_arp_entry(&EndpointRef::new("r", "b"), ip(192, 168, 1, 130))
            .unwrap(),
        Some(MacAddress([2, 0, 0, 0, 2, 2]))
    );
}

#[test]
fn unsupported_remote_command_without_gateway_does_not_leave_a_hidden_session() {
    let mut e = Engine::with_default_trace_capacity();
    host(
        &mut e,
        "h",
        ip(192, 168, 1, 10),
        None,
        MacAddress([0, 0, 0, 0, 1, 1]),
    );
    assert!(e.send_ping("h".into(), ip(10, 0, 0, 20), 1, 1).is_err());
    assert!(e.get_trace_snapshot().records.is_empty());
    e.configure_host_ipv4(
        EndpointRef::new("h", "eth0"),
        MacAddress([0, 0, 0, 0, 1, 1]),
        HostIpv4Config::new(
            Ipv4InterfaceConfig::new(ip(192, 168, 1, 10), 24).unwrap(),
            Some(ip(192, 168, 1, 1)),
        )
        .unwrap(),
        true,
    )
    .unwrap();
}

#[test]
fn router_retry_after_one_second_preserves_queued_ttl_and_original_rtt() {
    let mut e = s1_fixture();
    e.set_interface_admin_state(EndpointRef::new("host2", "eth0"), false)
        .unwrap();
    e.send_ping("host1".into(), ip(10, 0, 0, 20), 1, 1).unwrap();
    e.advance_until(VirtualTime(1_000_006_599), 1000).unwrap();
    e.set_interface_admin_state(EndpointRef::new("host2", "eth0"), true)
        .unwrap();
    e.run_until_quiescent(1000).unwrap();
    assert_eq!(e.now().0, 1_000_017_500);
    let trace = e.get_trace_snapshot();
    let requests: Vec<_> = trace
        .records
        .iter()
        .filter_map(|r| match &r.event {
            ObservableEvent::FrameTransmitted {
                from,
                frame:
                    EthernetFrame {
                        payload: EthernetPayload::Arp(a),
                        ..
                    },
                ..
            } if from == &EndpointRef::new("r1", "eth1")
                && a.operation == ArpOperation::Request =>
            {
                Some(r.at.0)
            }
            _ => None,
        })
        .collect();
    assert_eq!(requests, vec![6600, 1_000_006_600]);
    assert!(trace.records.iter().any(|r|matches!(&r.event,ObservableEvent::PingCompleted {host,rtt_ns:1_000_017_500,..} if host.0=="host1")));
    assert!(trace.records.iter().any(|r|matches!(&r.event,ObservableEvent::FrameTransmitted {from,frame:EthernetFrame {payload:EthernetPayload::Ipv4(p),..},..} if from==&EndpointRef::new("r1","eth1") && p.ttl==63)));
}

#[test]
fn resolved_router_retry_is_obsolete_and_configuration_remains_idle() {
    let mut e = s1_fixture();
    e.send_ping("host1".into(), ip(10, 0, 0, 20), 1, 1).unwrap();
    e.run_until_quiescent(1000).unwrap();
    let before = e.get_trace_snapshot();
    e.advance_until(VirtualTime(1_000_006_600), 1000).unwrap();
    assert_eq!(e.get_trace_snapshot(), before);
    e.add_router_static_route(
        &"r1".into(),
        spike_e1_rust::ipv4::Ipv4Network::new(ip(203, 0, 113, 0), 24).unwrap(),
        None,
        "eth0".into(),
        1,
    )
    .unwrap();
    assert_eq!(e.get_trace_snapshot(), before);
}
