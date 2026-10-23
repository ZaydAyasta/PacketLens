use super::*;
use crate::ipv4::{HostIpv4Config, Ipv4InterfaceConfig};
use crate::router_engine::RouterPortConfig;

fn ip(a: u8, b: u8, c: u8, d: u8) -> Ipv4Address {
    Ipv4Address([a, b, c, d])
}
fn fixture() -> Engine {
    let mut e = Engine::with_default_trace_capacity();
    e.create_host("h".into(), vec!["eth0".into()]).unwrap();
    e.configure_host_ipv4(
        EndpointRef::new("h", "eth0"),
        MacAddress([2, 0, 0, 0, 1, 1]),
        HostIpv4Config::new(
            Ipv4InterfaceConfig::new(ip(192, 168, 1, 10), 24).unwrap(),
            Some(ip(192, 168, 1, 1)),
        )
        .unwrap(),
        true,
    )
    .unwrap();
    e.create_router(
        "r".into(),
        vec![
            RouterPortConfig::new(
                "a".into(),
                MacAddress([2, 0, 0, 0, 2, 1]),
                Ipv4InterfaceConfig::new(ip(192, 168, 1, 1), 24).unwrap(),
                true,
            ),
            RouterPortConfig::new(
                "b".into(),
                MacAddress([2, 0, 0, 0, 2, 2]),
                Ipv4InterfaceConfig::new(ip(203, 0, 113, 1), 24).unwrap(),
                false,
            ),
        ],
    )
    .unwrap();
    e.connect_link(
        "l1".into(),
        EndpointRef::new("h", "eth0"),
        EndpointRef::new("r", "a"),
    )
    .unwrap();
    e
}
fn errors(e: &Engine) -> Vec<(u64, EndpointRef, Ipv4Packet)> {
    e.get_trace_snapshot()
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
            } if matches!(p.payload, Ipv4Payload::IcmpError(_)) => {
                Some((r.at.0, from.clone(), p.clone()))
            }
            _ => None,
        })
        .collect()
}
fn inject(
    e: &mut Engine,
    source: Ipv4Address,
    destination: Ipv4Address,
    ttl: u8,
    id: u16,
    seq: u16,
) -> EthernetFrame {
    let frame = EthernetFrame {
        src_mac: MacAddress([2, 0, 0, 0, 1, 1]),
        dst_mac: MacAddress([2, 0, 0, 0, 2, 1]),
        payload: EthernetPayload::Ipv4(Ipv4Packet {
            src_ip: source,
            dst_ip: destination,
            ttl,
            payload: Ipv4Payload::Icmp(IcmpPacket {
                kind: IcmpKind::EchoRequest,
                identifier: id,
                sequence: seq,
            }),
        }),
    };
    let dst = EndpointRef::new("r", "a");
    let instance = e.interface(&dst).unwrap().attached_link.unwrap();
    e.schedule(
        0,
        InternalEvent::FrameArrived {
            link_instance: instance,
            dst,
            frame: frame.clone(),
        },
    )
    .unwrap();
    frame
}

#[test]
fn transit_ttl_one_generates_time_exceeded() {
    for (ttl, id, seq, dst) in [(1, 7, 1, ip(198, 51, 100, 1)), (0, 17, 91, ip(10, 9, 8, 7))] {
        let mut e = fixture();
        let frame = inject(&mut e, ip(192, 168, 1, 10), dst, ttl, id, seq);
        assert_eq!(
            e.run_until_quiescent(1000).unwrap().outcome,
            ExecutionOutcome::Quiescent
        );
        let tx = errors(&e);
        assert_eq!(tx.len(), 1);
        assert_eq!(
            (tx[0].0, tx[0].2.src_ip, tx[0].2.dst_ip, tx[0].2.ttl),
            (2300, ip(192, 168, 1, 1), ip(192, 168, 1, 10), 64)
        );
        let Ipv4Payload::IcmpError(error) = &tx[0].2.payload else {
            panic!("error expected")
        };
        assert_eq!((error.icmp_type(), error.code()), (11, 0));
        assert_eq!(
            error.original(),
            &OriginalDatagramSummary {
                src_ip: ip(192, 168, 1, 10),
                dst_ip: dst,
                protocol: 1,
                echo_identifier: Some(id),
                echo_sequence: Some(seq)
            }
        );
        let trace = e.get_trace_snapshot();
        assert!(
            matches!(&trace.records[0].event,ObservableEvent::FrameDelivered {frame:observed,..} if observed==&frame)
        );
        let drops: Vec<_> = trace
            .records
            .iter()
            .filter(|r| matches!(r.event, ObservableEvent::PacketDropped { .. }))
            .collect();
        assert_eq!(drops.len(), 1);
        assert_eq!(drops[0].at.0, 0);
        assert!(
            matches!(&drops[0].event,ObservableEvent::PacketDropped {reason:DropReason::TtlExpiredInTransit,pdu:DroppedPdu::Ipv4(p),..} if p.ttl==ttl)
        );
        assert!(!trace.records.iter().any(|r| matches!(
            &r.event,
            ObservableEvent::FrameTransmitted {
                frame: EthernetFrame {
                    payload: EthernetPayload::Ipv4(Ipv4Packet {
                        payload: Ipv4Payload::Icmp(_),
                        ..
                    }),
                    ..
                },
                ..
            }
        )));
        assert_eq!(e.now().0, 3300);
    }
}

#[test]
fn missing_return_route_keeps_only_the_original_drop() {
    for ttl in [1, 64] {
        let mut e = fixture();
        inject(&mut e, ip(8, 8, 8, 8), ip(198, 51, 100, 1), ttl, 7, 2);
        e.run_until_quiescent(100).unwrap();
        let trace = e.get_trace_snapshot();
        assert!(errors(&e).is_empty());
        assert_eq!(trace.records.len(), 2);
        assert!(
            matches!(&trace.records[1].event,ObservableEvent::PacketDropped {reason,..} if *reason==if ttl==1 {DropReason::TtlExpiredInTransit} else {DropReason::NoRouteToHost})
        );
        assert_eq!(e.now().0, 0);
    }
}

#[test]
fn generated_error_uses_return_lpm_next_hop_and_egress_ip() {
    let mut e = fixture();
    e.set_interface_admin_state(EndpointRef::new("r", "b"), true)
        .unwrap();
    e.create_host("peer".into(), vec!["p".into()]).unwrap();
    e.configure_host_ipv4(
        EndpointRef::new("peer", "p"),
        MacAddress([2, 0, 0, 0, 3, 1]),
        HostIpv4Config::new(
            Ipv4InterfaceConfig::new(ip(203, 0, 113, 20), 24).unwrap(),
            None,
        )
        .unwrap(),
        true,
    )
    .unwrap();
    e.connect_link(
        "l2".into(),
        EndpointRef::new("r", "b"),
        EndpointRef::new("peer", "p"),
    )
    .unwrap();
    e.add_router_static_route(
        &"r".into(),
        crate::ipv4::Ipv4Network::new(ip(192, 168, 1, 10), 32).unwrap(),
        Some(ip(203, 0, 113, 20)),
        "b".into(),
        99,
    )
    .unwrap();
    inject(
        &mut e,
        ip(192, 168, 1, 10),
        ip(198, 51, 100, 99),
        64,
        65535,
        101,
    );
    e.run_until_quiescent(100).unwrap();
    let trace = e.get_trace_snapshot();
    let tx = errors(&e);
    assert_eq!(tx.len(), 1);
    assert_eq!(
        (tx[0].1.clone(), tx[0].2.src_ip, tx[0].2.dst_ip, tx[0].2.ttl),
        (
            EndpointRef::new("r", "b"),
            ip(203, 0, 113, 1),
            ip(192, 168, 1, 10),
            64
        )
    );
    assert!(trace.records.iter().any(|r|matches!(&r.event,ObservableEvent::ArpPacketQueued {target_ip,packet,..} if *target_ip==ip(203,0,113,20) && packet.dst_ip==ip(192,168,1,10) && packet.ttl==64 && matches!(packet.payload,Ipv4Payload::IcmpError(_)))));
    assert!(trace.records.iter().any(|r|matches!(&r.event,ObservableEvent::FrameTransmitted {from,frame,..} if from==&EndpointRef::new("r","b") && matches!(frame.payload,EthernetPayload::Ipv4(_)) && frame.src_mac==MacAddress([2,0,0,0,2,2]) && frame.dst_mac==MacAddress([2,0,0,0,3,1]))));
    assert!(!trace.records.iter().any(|r|matches!(&r.event,ObservableEvent::FrameDelivered {at_endpoint,frame:EthernetFrame {payload:EthernetPayload::Ipv4(p),..},..} if at_endpoint.device.0=="h" && matches!(p.payload,Ipv4Payload::IcmpError(_)))));
}

#[test]
fn generated_error_egress_failure_is_a_drop_not_a_delivery() {
    let mut e = fixture();
    e.send_ping("h".into(), ip(198, 51, 100, 1), 1, 1).unwrap();
    e.advance_until(VirtualTime(3200), 100).unwrap();
    e.set_interface_admin_state(EndpointRef::new("r", "a"), false)
        .unwrap();
    e.run_until_quiescent(100).unwrap();
    assert!(errors(&e).is_empty());
    let trace = e.get_trace_snapshot();
    assert_eq!(
        trace
            .records
            .iter()
            .filter(|r| matches!(r.event, ObservableEvent::PacketDropped { .. }))
            .count(),
        2
    );
    assert!(trace.records.iter().any(|r|r.at.0==3300 && matches!(&r.event,ObservableEvent::PacketDropped {reason:DropReason::EgressInterfaceDown,pdu:DroppedPdu::Ethernet(EthernetFrame {payload:EthernetPayload::Ipv4(p),..}),..} if matches!(p.payload,Ipv4Payload::IcmpError(_)))));
    assert_eq!(e.now().0, 3300);
}

#[test]
fn icmp_error_cannot_trigger_another_error_in_implemented_failure_paths() {
    for ttl in [1, 64] {
        for network_unreachable in [false, true] {
            let mut e = fixture();
            let dst = EndpointRef::new("r", "a");
            let link_instance = e.interface(&dst).unwrap().attached_link.unwrap();
            let original = OriginalDatagramSummary {
                src_ip: ip(1, 1, 1, 1),
                dst_ip: ip(2, 2, 2, 2),
                protocol: 1,
                echo_identifier: Some(7),
                echo_sequence: Some(1),
            };
            let packet = Ipv4Packet {
                src_ip: ip(192, 168, 1, 10),
                dst_ip: ip(198, 51, 100, 1),
                ttl,
                payload: Ipv4Payload::IcmpError(if network_unreachable {
                    IcmpErrorPacket::DestinationNetworkUnreachable { original }
                } else {
                    IcmpErrorPacket::TimeExceeded { original }
                }),
            };
            e.schedule(
                0,
                InternalEvent::FrameArrived {
                    link_instance,
                    dst,
                    frame: EthernetFrame {
                        src_mac: MacAddress([2, 0, 0, 0, 1, 1]),
                        dst_mac: MacAddress([2, 0, 0, 0, 2, 1]),
                        payload: EthernetPayload::Ipv4(packet.clone()),
                    },
                },
            )
            .unwrap();
            e.run_until_quiescent(100).unwrap();
            let trace = e.get_trace_snapshot();
            assert_eq!(trace.records.len(), 2);
            assert!(errors(&e).is_empty());
            assert!(
                matches!(&trace.records[1].event,ObservableEvent::PacketDropped {pdu:DroppedPdu::Ipv4(p),..} if matches!(p.payload,Ipv4Payload::IcmpError(_)))
            );
            assert_eq!(e.now().0, 0);
        }
    }
}

#[test]
fn generated_error_arp_failure_drops_it_without_secondary_error() {
    let mut e = fixture();
    e.set_interface_admin_state(EndpointRef::new("h", "eth0"), false)
        .unwrap();
    inject(&mut e, ip(192, 168, 1, 10), ip(198, 51, 100, 1), 1, 7, 1);
    e.run_until_quiescent(1000).unwrap();
    assert!(errors(&e).is_empty());
    let trace = e.get_trace_snapshot();
    let drops:Vec<_>=trace.records.iter().filter(|r|matches!(&r.event,ObservableEvent::PacketDropped {pdu:DroppedPdu::Ipv4(p),reason:DropReason::ArpResolutionFailed,..} if matches!(p.payload,Ipv4Payload::IcmpError(_)))).collect();
    assert_eq!(drops.len(), 1);
    assert_eq!(drops[0].at.0, 3_000_000_100);
    assert!(matches!(&drops[0].event, ObservableEvent::PacketDropped {
        pdu: DroppedPdu::Ipv4(packet), ..
    } if packet.ttl == 64 && matches!(&packet.payload,
        Ipv4Payload::IcmpError(IcmpErrorPacket::TimeExceeded { original })
            if *original == OriginalDatagramSummary {
                src_ip: ip(192, 168, 1, 10),
                dst_ip: ip(198, 51, 100, 1),
                protocol: 1,
                echo_identifier: Some(7),
                echo_sequence: Some(1),
            })));
    assert_eq!(trace.records.iter().filter(|r|matches!(&r.event,ObservableEvent::FrameTransmitted {frame:EthernetFrame {payload:EthernetPayload::Arp(a),..},..} if a.operation==ArpOperation::Request)).count(),3);
    e.add_router_static_route(
        &"r".into(),
        crate::ipv4::Ipv4Network::new(ip(10, 0, 0, 0), 24).unwrap(),
        None,
        "a".into(),
        1,
    )
    .unwrap();
}

fn loop_return_fixture() -> Engine {
    let mut e = Engine::with_default_trace_capacity();
    e.create_host("h".into(), vec!["eth0".into()]).unwrap();
    e.configure_host_ipv4(
        EndpointRef::new("h", "eth0"),
        MacAddress([2, 0, 0, 0, 1, 1]),
        HostIpv4Config::new(
            Ipv4InterfaceConfig::new(ip(192, 168, 1, 10), 24).unwrap(),
            Some(ip(192, 168, 1, 1)),
        )
        .unwrap(),
        true,
    )
    .unwrap();
    for (router, mac, first, first_prefix, second, second_prefix, second_up) in [
        ("r1", 2, ip(192, 168, 1, 1), 24, ip(172, 16, 0, 1), 30, true),
        (
            "r2",
            3,
            ip(172, 16, 0, 2),
            30,
            ip(203, 0, 113, 1),
            24,
            false,
        ),
    ] {
        e.create_router(
            router.into(),
            vec![
                RouterPortConfig::new(
                    "eth0".into(),
                    MacAddress([2, 0, 0, 0, mac, 1]),
                    Ipv4InterfaceConfig::new(first, first_prefix).unwrap(),
                    true,
                ),
                RouterPortConfig::new(
                    "eth1".into(),
                    MacAddress([2, 0, 0, 0, mac, 2]),
                    Ipv4InterfaceConfig::new(second, second_prefix).unwrap(),
                    second_up,
                ),
            ],
        )
        .unwrap();
    }
    e.connect_link(
        "l1".into(),
        EndpointRef::new("h", "eth0"),
        EndpointRef::new("r1", "eth0"),
    )
    .unwrap();
    e.connect_link(
        "l2".into(),
        EndpointRef::new("r1", "eth1"),
        EndpointRef::new("r2", "eth0"),
    )
    .unwrap();
    for (r, net, prefix, hop, port) in [
        ("r1", ip(10, 0, 0, 0), 24, ip(172, 16, 0, 2), "eth1"),
        ("r2", ip(10, 0, 0, 0), 24, ip(172, 16, 0, 1), "eth0"),
        ("r2", ip(192, 168, 1, 0), 24, ip(172, 16, 0, 1), "eth0"),
    ] {
        e.add_router_static_route(
            &r.into(),
            crate::ipv4::Ipv4Network::new(net, prefix).unwrap(),
            Some(hop),
            port.into(),
            10,
        )
        .unwrap();
    }
    // Controlled normal reception, not an accepted SendPing: host TTL configuration is a later increment.
    let dst = EndpointRef::new("r1", "eth0");
    let link_instance = e.interface(&dst).unwrap().attached_link.unwrap();
    e.schedule(
        0,
        InternalEvent::FrameArrived {
            link_instance,
            dst,
            frame: EthernetFrame {
                src_mac: MacAddress([2, 0, 0, 0, 1, 1]),
                dst_mac: MacAddress([2, 0, 0, 0, 2, 1]),
                payload: EthernetPayload::Ipv4(Ipv4Packet {
                    src_ip: ip(192, 168, 1, 10),
                    dst_ip: ip(10, 0, 0, 20),
                    ttl: 4,
                    payload: Ipv4Payload::Icmp(IcmpPacket {
                        kind: IcmpKind::EchoRequest,
                        identifier: 1,
                        sequence: 1,
                    }),
                }),
            },
        },
    )
    .unwrap();
    e
}

#[test]
fn time_exceeded_returns_across_router_with_intact_summary() {
    let mut e = loop_return_fixture();
    assert_eq!(
        e.run_until_quiescent(1000).unwrap().outcome,
        ExecutionOutcome::Quiescent
    );
    let trace = e.get_trace_snapshot();
    let ttls: Vec<_> = trace
        .records
        .iter()
        .filter_map(|r| match &r.event {
            ObservableEvent::FrameTransmitted {
                frame:
                    EthernetFrame {
                        payload: EthernetPayload::Ipv4(p),
                        ..
                    },
                ..
            } if matches!(p.payload, Ipv4Payload::Icmp(_)) => Some(p.ttl),
            _ => None,
        })
        .collect();
    assert_eq!(ttls, vec![3, 2, 1]);
    let tx = errors(&e);
    assert_eq!(tx.len(), 2);
    assert_eq!(
        (tx[0].1.clone(), tx[0].2.src_ip, tx[0].2.ttl),
        (EndpointRef::new("r2", "eth0"), ip(172, 16, 0, 2), 64)
    );
    assert_eq!(
        (tx[1].1.clone(), tx[1].2.src_ip, tx[1].2.ttl),
        (EndpointRef::new("r1", "eth0"), ip(172, 16, 0, 2), 63)
    );
    for (_, _, p) in tx {
        let Ipv4Payload::IcmpError(error) = p.payload else {
            panic!("error expected")
        };
        assert_eq!((error.icmp_type(), error.code()), (11, 0));
        assert_eq!(
            error.original(),
            &OriginalDatagramSummary {
                src_ip: ip(192, 168, 1, 10),
                dst_ip: ip(10, 0, 0, 20),
                protocol: 1,
                echo_identifier: Some(1),
                echo_sequence: Some(1)
            }
        );
    }
    assert_eq!(trace.records.iter().filter(|r|matches!(&r.event,ObservableEvent::PacketDropped {at_device,reason:DropReason::TtlExpiredInTransit,pdu:DroppedPdu::Ipv4(p),..} if at_device.0=="r2" && p.ttl==1)).count(),1);
    assert_eq!(e.now().0, 9900);
}

pub(crate) fn observation_projection(e: &Engine, label: &str) -> Vec<String> {
    fn address(a: Ipv4Address) -> String {
        format!("{}.{}.{}.{}", a.0[0], a.0[1], a.0[2], a.0[3])
    }
    fn mac(m: MacAddress) -> String {
        m.0.iter()
            .map(|b| format!("{b:02x}"))
            .collect::<Vec<_>>()
            .join(":")
    }
    fn ep(p: &EndpointRef) -> String {
        format!("{}:{}", p.device.0, p.interface.0)
    }
    fn packet(p: &Ipv4Packet) -> String {
        let message = match &p.payload {
            Ipv4Payload::Icmp(c) => format!("echo,{:?},{},{}", c.kind, c.identifier, c.sequence),
            Ipv4Payload::IcmpError(error) => {
                let s = error.original();
                format!(
                    "error,{},{},{},{},{},{},{}",
                    error.icmp_type(),
                    error.code(),
                    address(s.src_ip),
                    address(s.dst_ip),
                    s.protocol,
                    s.echo_identifier
                        .map(|n| n.to_string())
                        .unwrap_or("-".into()),
                    s.echo_sequence.map(|n| n.to_string()).unwrap_or("-".into())
                )
            }
        };
        format!(
            "ip,{},{},{},{}",
            address(p.src_ip),
            address(p.dst_ip),
            p.ttl,
            message
        )
    }
    fn frame(f: &EthernetFrame) -> String {
        let data = match &f.payload {
            EthernetPayload::Ipv4(p) => packet(p),
            EthernetPayload::Arp(a) => format!(
                "arp,{:?},{},{},{},{}",
                a.operation,
                address(a.sender_ip),
                mac(a.sender_mac),
                address(a.target_ip),
                mac(a.target_mac)
            ),
        };
        format!("eth,{},{},{}", mac(f.src_mac), mac(f.dst_mac), data)
    }
    let trace = e.get_trace_snapshot();
    let mut rows = vec![format!(
        "ICMP4|{label}|result|{}|{}|{}",
        e.now().0,
        trace.total_emitted_ever,
        trace.dropped_by_capacity
    )];
    for r in trace.records {
        let detail = match r.event {
            ObservableEvent::FrameTransmitted {
                link,
                from,
                to,
                arrives_at,
                frame: f,
            } => format!(
                "tx|{}|{}|{}|{}|{}",
                link.0,
                ep(&from),
                ep(&to),
                arrives_at.0,
                frame(&f)
            ),
            ObservableEvent::FrameDelivered {
                link,
                at_endpoint,
                frame: f,
            } => format!("rx|{}|{}|{}", link.0, ep(&at_endpoint), frame(&f)),
            ObservableEvent::ArpPacketQueued {
                at_device,
                target_ip,
                packet: p,
            } => format!(
                "queue|{}|{}|{}",
                at_device.0,
                address(target_ip),
                packet(&p)
            ),
            ObservableEvent::TableUpdated {
                at_device,
                update:
                    TableUpdateDetail::ArpLearned {
                        ip,
                        mac: m,
                        expires_at,
                    },
            } => format!(
                "arp|{}|{}|{}|{}",
                at_device.0,
                address(ip),
                mac(m),
                expires_at.0
            ),
            ObservableEvent::PacketDropped {
                at_device,
                at_interface,
                pdu,
                reason,
            } => format!(
                "drop|{}|{}|{:?}|{}",
                at_device.0,
                at_interface.map(|i| i.0).unwrap_or("-".into()),
                reason,
                match pdu {
                    DroppedPdu::Ethernet(f) => frame(&f),
                    DroppedPdu::Ipv4(p) => packet(&p),
                }
            ),
            ObservableEvent::PingFailed {
                host,
                target_ip,
                identifier,
                sequence,
                reason,
            } => format!(
                "failed|{}|{}|{}|{}|{reason:?}",
                host.0,
                address(target_ip),
                identifier,
                sequence
            ),
            ObservableEvent::PingCompleted {
                host,
                target_ip,
                identifier,
                sequence,
                rtt_ns,
            } => format!(
                "completed|{}|{}|{}|{}|{}",
                host.0,
                address(target_ip),
                identifier,
                sequence,
                rtt_ns
            ),
            ObservableEvent::TableUpdated {
                at_device,
                update: TableUpdateDetail::ArpEntryExpired { ip },
            } => format!("expired|{}|{}", at_device.0, address(ip)),
            other => panic!("unexpected observation in error-only probe: {other:?}"),
        };
        rows.push(format!(
            "ICMP4|{label}|{}|{}|{}|{detail}",
            r.seq.0,
            r.at.0,
            r.observation_interface.map(|i| i.0).unwrap_or("-".into())
        ));
    }
    rows
}

fn differential_case(label: &str) -> Engine {
    let mut e = if label == "loop-return" {
        loop_return_fixture()
    } else {
        fixture()
    };
    match label {
        "no-route" | "egress-failure" => {
            e.send_ping("h".into(), ip(198, 51, 100, 1), 7, 2).unwrap();
            if label == "egress-failure" {
                e.advance_until(VirtualTime(3200), 100).unwrap();
                e.set_interface_admin_state(EndpointRef::new("r", "a"), false)
                    .unwrap();
            }
        }
        "ttl-one" => {
            inject(&mut e, ip(192, 168, 1, 10), ip(10, 0, 0, 20), 1, 65535, 23);
        }
        "ttl-zero" => {
            inject(&mut e, ip(192, 168, 1, 10), ip(203, 0, 113, 99), 0, 8, 4);
        }
        "no-return" => {
            inject(&mut e, ip(8, 8, 8, 8), ip(198, 51, 100, 1), 64, 9, 3);
        }
        "arp-failure" => {
            e.set_interface_admin_state(EndpointRef::new("h", "eth0"), false)
                .unwrap();
            inject(&mut e, ip(192, 168, 1, 10), ip(198, 51, 100, 1), 1, 7, 1);
        }
        "loop-return" => {}
        _ => panic!("unknown scenario"),
    }
    assert_eq!(
        e.run_until_quiescent(1000).unwrap().outcome,
        ExecutionOutcome::Quiescent
    );
    e
}

#[test]
fn icmp_cross_projection_is_deterministic() {
    for label in [
        "no-route",
        "ttl-one",
        "ttl-zero",
        "no-return",
        "egress-failure",
        "arp-failure",
        "loop-return",
    ] {
        let expected = observation_projection(&differential_case(label), label);
        for _ in 1..100 {
            assert_eq!(
                observation_projection(&differential_case(label), label),
                expected
            );
        }
        for row in expected {
            println!("{row}");
        }
    }
}

#[test]
fn ordinary_router_arp_exhaustion_returns_host_unreachable() {
    let mut e = loop_return_fixture();
    e.set_interface_admin_state(EndpointRef::new("r2", "eth0"), false)
        .unwrap();
    assert_eq!(
        e.run_until_quiescent(1000).unwrap().outcome,
        ExecutionOutcome::Quiescent
    );
    let tx = errors(&e);
    assert_eq!(tx.len(), 1);
    assert!(matches!(
        &tx[0].2.payload,
        Ipv4Payload::IcmpError(IcmpErrorPacket::DestinationHostUnreachable { .. })
    ));
    assert_eq!(
        e.get_trace_snapshot()
            .records
            .iter()
            .filter(|r| matches!(
                &r.event,
                ObservableEvent::PacketDropped {
                    reason: DropReason::ArpResolutionFailed,
                    ..
                }
            ))
            .count(),
        1
    );
}
#[test]
fn no_route_generates_network_unreachable() {
    for (id, seq, dst) in [(1, 1, ip(198, 51, 100, 1)), (19, 41, ip(10, 99, 8, 7))] {
        let mut e = fixture();
        e.send_ping("h".into(), dst, id, seq).unwrap();
        assert_eq!(
            e.run_until_quiescent(1000).unwrap().outcome,
            ExecutionOutcome::Quiescent
        );
        let tx = errors(&e);
        assert_eq!(tx.len(), 1);
        assert_eq!(
            (tx[0].0, tx[0].1.clone()),
            (3300, EndpointRef::new("r", "a"))
        );
        let p = &tx[0].2;
        assert_eq!(
            (p.src_ip, p.dst_ip, p.ttl),
            (ip(192, 168, 1, 1), ip(192, 168, 1, 10), 64)
        );
        let Ipv4Payload::IcmpError(error) = &p.payload else {
            panic!("error expected")
        };
        assert_eq!((error.icmp_type(), error.code()), (3, 0));
        assert_eq!(
            error.original(),
            &OriginalDatagramSummary {
                src_ip: ip(192, 168, 1, 10),
                dst_ip: dst,
                protocol: 1,
                echo_identifier: Some(id),
                echo_sequence: Some(seq)
            }
        );
        let trace = e.get_trace_snapshot();
        let drops: Vec<_> = trace
            .records
            .iter()
            .filter(|r| matches!(r.event, ObservableEvent::PacketDropped { .. }))
            .collect();
        assert_eq!(drops.len(), 1);
        assert_eq!(drops[0].at.0, 3200);
        let error_tx = trace
            .records
            .iter()
            .find(|r| {
                matches!(&r.event,
            ObservableEvent::FrameTransmitted {
                frame: EthernetFrame { payload: EthernetPayload::Ipv4(packet), .. }, ..
            } if matches!(packet.payload, Ipv4Payload::IcmpError(_)))
            })
            .unwrap();
        assert!(drops[0].seq < error_tx.seq);
        assert!(
            matches!(&drops[0].event,ObservableEvent::PacketDropped {pdu:DroppedPdu::Ipv4(packet),reason:DropReason::NoRouteToHost,..} if packet.ttl==63)
        );
        assert!(trace.records.iter().any(|r|matches!(&r.event,ObservableEvent::FrameDelivered {at_endpoint,frame:EthernetFrame {payload:EthernetPayload::Ipv4(packet),..},..} if at_endpoint.device.0=="r" && packet.ttl==64)));
        assert!(trace.records.iter().any(|r|matches!(&r.event,ObservableEvent::FrameDelivered {at_endpoint,frame:EthernetFrame {payload:EthernetPayload::Ipv4(packet),..},..} if at_endpoint.device.0=="h" && matches!(packet.payload,Ipv4Payload::IcmpError(_)))));
        assert!(!trace.records.iter().any(|r| matches!(
            r.event,
            ObservableEvent::PingCompleted { .. } | ObservableEvent::PingFailed { .. }
        )));
        assert_eq!(e.now().0, 4300);
    }
}
