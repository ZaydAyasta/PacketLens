use super::*;
use crate::ipv4::{HostIpv4Config, Ipv4InterfaceConfig};
use crate::router_engine::RouterPortConfig;

const H: Ipv4Address = Ipv4Address([192, 168, 1, 10]);
const R: Ipv4Address = Ipv4Address([192, 168, 1, 1]);
const TARGET: Ipv4Address = Ipv4Address([10, 0, 0, 20]);
fn ep(d: &str, p: &str) -> EndpointRef {
    EndpointRef::new(d, p)
}
fn mac(n: u8) -> MacAddress {
    MacAddress([2, 0, 0, 0, 0, n])
}
fn fixture() -> Engine {
    let mut e = Engine::with_default_trace_capacity();
    for (name, ip, m, gateway) in [
        ("h", H, 1, Some(R)),
        ("peer", Ipv4Address([10, 0, 0, 30]), 4, None),
    ] {
        e.create_host(name.into(), vec!["p".into()]).unwrap();
        e.configure_host_ipv4(
            ep(name, "p"),
            mac(m),
            HostIpv4Config::new(Ipv4InterfaceConfig::new(ip, 24).unwrap(), gateway).unwrap(),
            true,
        )
        .unwrap();
    }
    e.create_router(
        "r".into(),
        vec![
            RouterPortConfig::new(
                "a".into(),
                mac(2),
                Ipv4InterfaceConfig::new(R, 24).unwrap(),
                true,
            ),
            RouterPortConfig::new(
                "b".into(),
                mac(3),
                Ipv4InterfaceConfig::new(Ipv4Address([10, 0, 0, 1]), 24).unwrap(),
                true,
            ),
        ],
    )
    .unwrap();
    e.connect_link("l1".into(), ep("h", "p"), ep("r", "a"))
        .unwrap();
    e.connect_link("l2".into(), ep("r", "b"), ep("peer", "p"))
        .unwrap();
    e
}
fn request(id: u16) -> Ipv4Packet {
    Ipv4Packet {
        src_ip: H,
        dst_ip: TARGET,
        ttl: 64,
        payload: Ipv4Payload::Icmp(IcmpPacket {
            kind: IcmpKind::EchoRequest,
            identifier: id,
            sequence: 17,
        }),
    }
}
fn inject(e: &mut Engine, endpoint: EndpointRef, packet: Ipv4Packet, at: u64) {
    let link_instance = e.interface(&endpoint).unwrap().attached_link.unwrap();
    let destination = e.interface(&endpoint).unwrap().mac.unwrap();
    e.schedule(
        at,
        InternalEvent::FrameArrived {
            link_instance,
            dst: endpoint,
            frame: EthernetFrame {
                src_mac: mac(1),
                dst_mac: destination,
                payload: EthernetPayload::Ipv4(packet),
            },
        },
    )
    .unwrap();
}
fn drops(e: &Engine) -> Vec<(u64, InterfaceName, Ipv4Packet, DropReason)> {
    e.get_trace_snapshot()
        .records
        .iter()
        .filter_map(|r| match &r.event {
            ObservableEvent::PacketDropped {
                at_interface,
                pdu: DroppedPdu::Ipv4(p),
                reason,
                ..
            } => Some((
                r.at.0,
                at_interface.clone().unwrap_or_else(|| "".into()),
                p.clone(),
                *reason,
            )),
            _ => None,
        })
        .collect()
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
fn assert_idle(e: &mut Engine) {
    assert!(e.pings.is_empty());
    for port in [ep("r", "a"), ep("r", "b")] {
        let arp = e.router_arp_mut(&port).unwrap();
        assert!(arp.pending.is_empty());
        assert!(arp.resolutions.is_empty());
    }
    e.add_router_static_route(
        &"r".into(),
        crate::ipv4::Ipv4Network::new(Ipv4Address([198, 19, 0, 0]), 24).unwrap(),
        None,
        "a".into(),
        1,
    )
    .unwrap();
}
#[test]
fn router_arp_timeout_drains_fifo() {
    let mut e = fixture();
    for id in [3, 1, 2] {
        inject(&mut e, ep("r", "a"), request(id), 0);
    }
    assert_eq!(
        e.run_until_quiescent(1000).unwrap().outcome,
        ExecutionOutcome::Quiescent
    );
    let ds = drops(&e);
    assert_eq!(ds.len(), 3);
    let trace = e.get_trace_snapshot();
    let requests: Vec<_> = trace
        .records
        .iter()
        .filter_map(|r| match &r.event {
            ObservableEvent::FrameTransmitted {
                from,
                frame:
                    EthernetFrame {
                        payload: EthernetPayload::Arp(arp),
                        ..
                    },
                ..
            } if *from == ep("r", "b")
                && arp.operation == ArpOperation::Request
                && arp.target_ip == TARGET =>
            {
                Some(r.at.0)
            }
            _ => None,
        })
        .collect();
    assert_eq!(requests, vec![100, 1_000_000_100, 2_000_000_100]);
    for (d, id) in ds.iter().zip([3, 1, 2]) {
        let mut expected = request(id);
        expected.ttl = 63;
        assert_eq!(
            d,
            &(
                3_000_000_100,
                "b".into(),
                expected,
                DropReason::ArpResolutionFailed
            )
        );
    }
    assert_eq!(errors(&e).len(), 3);
    assert_idle(&mut e);
}
#[test]
fn router_arp_timeout_generates_eligible_host_unreachable() {
    for id in [7, 65535] {
        let mut e = fixture();
        let p = request(id);
        inject(&mut e, ep("r", "a"), p.clone(), 0);
        e.run_until_quiescent(1000).unwrap();
        let tx = errors(&e);
        assert_eq!(tx.len(), 1);
        assert_eq!(
            (
                tx[0].0,
                tx[0].1.clone(),
                tx[0].2.src_ip,
                tx[0].2.dst_ip,
                tx[0].2.ttl
            ),
            (3_000_002_400, ep("r", "a"), R, H, 64)
        );
        assert_eq!(
            tx[0].2.payload,
            Ipv4Payload::IcmpError(IcmpErrorPacket::DestinationHostUnreachable {
                original: OriginalDatagramSummary {
                    src_ip: H,
                    dst_ip: TARGET,
                    protocol: 1,
                    echo_identifier: Some(id),
                    echo_sequence: Some(17)
                }
            })
        );
        assert_eq!(e.now().0, 3_000_003_400);
        assert_idle(&mut e);
    }
}
#[test]
fn mixed_arp_queue_never_generates_error_on_error() {
    let mut e = fixture();
    let a = request(3);
    let mut b = request(99);
    b.payload = Ipv4Payload::IcmpError(IcmpErrorPacket::TimeExceeded {
        original: OriginalDatagramSummary::from_ipv4(&a),
    });
    for p in [a.clone(), b.clone(), request(2)] {
        inject(&mut e, ep("r", "a"), p, 0);
    }
    e.run_until_quiescent(1000).unwrap();
    let ds = drops(&e);
    assert_eq!(ds.len(), 3);
    for (d, mut p) in ds.iter().zip([a, b, request(2)]) {
        p.ttl = 63;
        assert_eq!(d.2, p);
        assert_eq!(d.3, DropReason::ArpResolutionFailed);
    }
    let tx = errors(&e);
    assert_eq!(tx.len(), 2);
    assert_eq!(
        tx.iter()
            .map(|t| match &t.2.payload {
                Ipv4Payload::IcmpError(x) => x.original().echo_identifier,
                _ => None,
            })
            .collect::<Vec<_>>(),
        vec![Some(3), Some(2)]
    );
    assert_idle(&mut e);
}
#[test]
fn error_arp_failure_does_not_recurse() {
    let mut e = fixture();
    let mut p = request(1);
    p.src_ip = Ipv4Address([192, 168, 1, 99]);
    inject(&mut e, ep("r", "a"), p.clone(), 0);
    e.run_until_quiescent(1000).unwrap();
    let ds = drops(&e);
    assert_eq!(ds.len(), 2);
    assert_eq!(ds[0].0, 3_000_000_100);
    assert_eq!(ds[1].0, 6_000_000_200);
    assert_eq!(ds[0].3, DropReason::ArpResolutionFailed);
    assert_eq!(ds[1].3, DropReason::ArpResolutionFailed);
    assert_eq!(
        ds[1].2.payload,
        Ipv4Payload::IcmpError(IcmpErrorPacket::DestinationHostUnreachable {
            original: OriginalDatagramSummary {
                src_ip: Ipv4Address([192, 168, 1, 99]),
                dst_ip: TARGET,
                protocol: 1,
                echo_identifier: Some(1),
                echo_sequence: Some(17)
            }
        })
    );
    assert!(errors(&e).is_empty());
    assert_idle(&mut e);
}
#[test]
fn reply_without_return_route_has_normative_drop() {
    for receiver in [ep("h", "p"), ep("r", "a")] {
        let mut e = fixture();
        let mut p = request(7);
        p.src_ip = Ipv4Address([198, 18, 0, 10]);
        p.dst_ip = if receiver.device.0 == "h" { H } else { R };
        p.ttl = 1;
        if receiver.device.0 == "h" {
            e.configure_host_ipv4(
                receiver.clone(),
                mac(1),
                HostIpv4Config::new(Ipv4InterfaceConfig::new(H, 24).unwrap(), None).unwrap(),
                true,
            )
            .unwrap();
        }
        inject(&mut e, receiver.clone(), p.clone(), 0);
        e.run_until_quiescent(1000).unwrap();
        let ds = drops(&e);
        assert_eq!(ds.len(), 1);
        assert_eq!(ds[0].3, DropReason::NoRouteToHost);
        assert_eq!(ds[0].0, 0);
        assert_eq!(
            ds[0].2,
            Ipv4Packet {
                src_ip: p.dst_ip,
                dst_ip: p.src_ip,
                ttl: 64,
                payload: Ipv4Payload::Icmp(IcmpPacket {
                    kind: IcmpKind::EchoReply,
                    identifier: 7,
                    sequence: 17
                })
            }
        );
        assert!(errors(&e).is_empty());
        assert_idle(&mut e);
    }
}
#[test]
fn failed_reply_creates_no_receiver_ping() {
    for gateway in [None, Some(Ipv4Address([10, 0, 0, 99]))] {
        let mut e = fixture();
        e.configure_host_ipv4(
            ep("peer", "p"),
            mac(4),
            HostIpv4Config::new(Ipv4InterfaceConfig::new(TARGET, 24).unwrap(), gateway).unwrap(),
            true,
        )
        .unwrap();
        e.send_ping("h".into(), TARGET, 5, 9).unwrap();
        e.run_until_quiescent(1000).unwrap();
        let ds = drops(&e);
        assert_eq!(ds.len(), 1);
        let d = &ds[0];
        assert_eq!(
            d.0,
            if gateway.is_none() {
                6500
            } else {
                3_000_006_600
            }
        );
        assert_eq!(
            d.3,
            if gateway.is_none() {
                DropReason::NoRouteToHost
            } else {
                DropReason::ArpResolutionFailed
            }
        );
        assert_eq!(
            d.2,
            Ipv4Packet {
                src_ip: TARGET,
                dst_ip: H,
                ttl: 64,
                payload: Ipv4Payload::Icmp(IcmpPacket {
                    kind: IcmpKind::EchoReply,
                    identifier: 5,
                    sequence: 9
                })
            }
        );
        assert_eq!(e.pings.len(), 1);
        assert!(e.pings.keys().all(|k| k.source.0 == "h"));
        assert!(!e.get_trace_snapshot().records.iter().any(|r| matches!(
            r.event,
            ObservableEvent::PingCompleted { .. } | ObservableEvent::PingFailed { .. }
        )));
    }
}
#[test]
fn s3_no_gateway_is_drop_and_one_terminal_without_hidden_session() {
    let mut e = fixture();
    e.configure_host_ipv4(
        ep("h", "p"),
        mac(1),
        HostIpv4Config::new(Ipv4InterfaceConfig::new(H, 24).unwrap(), None).unwrap(),
        true,
    )
    .unwrap();
    for _ in 0..2 {
        e.send_ping("h".into(), TARGET, 4, 1).unwrap();
        assert_eq!(
            e.run_until_quiescent(100).unwrap().outcome,
            ExecutionOutcome::Quiescent
        );
    }
    let trace = e.get_trace_snapshot();
    assert_eq!(trace.records.len(), 4);
    assert_eq!(e.now().0, 0);
    for pair in trace.records.chunks(2) {
        assert!(
            matches!(&pair[0].event,ObservableEvent::PacketDropped {pdu:DroppedPdu::Ipv4(p),reason:DropReason::NoRouteToHost,..} if p.dst_ip==TARGET&&p.ttl==64)
        );
        match &pair[1].event {
            ObservableEvent::PingFailed {
                host,
                target_ip,
                identifier,
                sequence,
                reason,
            } => {
                assert_eq!(host.0, "h");
                assert_eq!((*target_ip, *identifier, *sequence), (TARGET, 4, 1));
                assert_eq!(*reason, PingFailureReason::DestinationNetworkUnreachable);
            }
            _ => panic!("terminal expected"),
        }
    }
    assert_idle(&mut e);
}

fn run_case(label: &str) -> Engine {
    let mut e = fixture();
    let a = ep("r", "a");
    let b = ep("r", "b");
    if label.starts_with("s7-") {
        let gateway = if label == "s7-no-gateway" {
            None
        } else {
            Some(Ipv4Address([10, 0, 0, 99]))
        };
        e.configure_host_ipv4(
            ep("peer", "p"),
            mac(4),
            HostIpv4Config::new(Ipv4InterfaceConfig::new(TARGET, 24).unwrap(), gateway).unwrap(),
            true,
        )
        .unwrap();
        e.send_ping("h".into(), TARGET, 5, 9).unwrap();
        assert_eq!(
            e.run_until_quiescent(1000).unwrap().outcome,
            ExecutionOutcome::Quiescent
        );
        let ds = drops(&e);
        assert_eq!(ds.len(), 1);
        assert_eq!(
            ds[0].0,
            if gateway.is_none() {
                6500
            } else {
                3_000_006_600
            }
        );
        assert_eq!(
            ds[0].3,
            if gateway.is_none() {
                DropReason::NoRouteToHost
            } else {
                DropReason::ArpResolutionFailed
            }
        );
        assert_eq!(
            ds[0].2,
            Ipv4Packet {
                src_ip: TARGET,
                dst_ip: H,
                ttl: 64,
                payload: Ipv4Payload::Icmp(IcmpPacket {
                    kind: IcmpKind::EchoReply,
                    identifier: 5,
                    sequence: 9
                })
            }
        );
        assert_eq!(e.pings.len(), 1);
        assert!(e.pings.keys().all(|k| k.source.0 == "h"));
        assert!(errors(&e).is_empty());
        assert!(!e.get_trace_snapshot().records.iter().any(|r| matches!(
            &r.event,
            ObservableEvent::PingFailed { .. } | ObservableEvent::PingCompleted { .. }
        )));
        return e;
    }
    if label.starts_with("reply-") {
        let receiver = if label.contains("host") {
            ep("h", "p")
        } else {
            a.clone()
        };
        let source = if label.contains("exhaust") {
            Ipv4Address([192, 168, 1, 99])
        } else if receiver.device.0 == "h" {
            R
        } else {
            H
        };
        let own = if receiver.device.0 == "h" { H } else { R };
        for id in [3, 1, 2] {
            let mut p = request(id);
            p.src_ip = source;
            p.dst_ip = own;
            p.ttl = 1;
            inject(&mut e, receiver.clone(), p, 0);
        }
        if !label.contains("exhaust") {
            let at = if label.contains("flight") { 2300 } else { 2200 };
            e.advance_until(VirtualTime(at), 1000).unwrap();
            if label.contains("down") {
                e.set_interface_admin_state(receiver.clone(), false)
                    .unwrap();
            } else {
                e.disconnect_link(&"l1".into()).unwrap();
                if label.contains("reconnect") {
                    e.connect_link("l-new".into(), ep("h", "p"), a.clone())
                        .unwrap();
                }
            }
        }
        assert_eq!(
            e.run_until_quiescent(1000).unwrap().outcome,
            ExecutionOutcome::Quiescent
        );
        assert!(errors(&e).is_empty());
        assert!(e.pings.is_empty());
        let trace = e.get_trace_snapshot();
        let replies: Vec<_> = trace
            .records
            .iter()
            .filter_map(|r| match &r.event {
                ObservableEvent::PacketDropped { reason, pdu, .. } => {
                    let p = match pdu {
                        DroppedPdu::Ipv4(p) => Some(p),
                        DroppedPdu::Ethernet(EthernetFrame {
                            payload: EthernetPayload::Ipv4(p),
                            ..
                        }) => Some(p),
                        _ => None,
                    };
                    p.map(|p| (r.at.0, *reason, p.clone()))
                }
                _ => None,
            })
            .collect();
        if label.contains("reconnect") {
            assert!(replies.is_empty());
            assert_eq!(e.now().0, 3300);
            assert_eq!(
                trace
                    .records
                    .iter()
                    .filter(|r| matches!(
                        &r.event,
                        ObservableEvent::FrameTransmitted {
                            frame: EthernetFrame {
                                payload: EthernetPayload::Ipv4(_),
                                ..
                            },
                            ..
                        }
                    ))
                    .count(),
                3
            );
        } else {
            assert_eq!(replies.len(), 3);
            let reason = if label.contains("exhaust") {
                DropReason::ArpResolutionFailed
            } else if label.contains("down") {
                DropReason::EgressInterfaceDown
            } else if label.contains("flight") {
                DropReason::LinkDisconnectedInFlight
            } else {
                DropReason::EgressLinkUnavailable
            };
            let time = if label.contains("exhaust") {
                3_000_000_100
            } else if label.contains("flight") {
                3300
            } else {
                2300
            };
            for (d, id) in replies.iter().zip([3, 1, 2]) {
                assert_eq!((d.0, d.1), (time, reason));
                assert_eq!(
                    d.2,
                    Ipv4Packet {
                        src_ip: own,
                        dst_ip: source,
                        ttl: 64,
                        payload: Ipv4Payload::Icmp(IcmpPacket {
                            kind: IcmpKind::EchoReply,
                            identifier: id,
                            sequence: 17
                        })
                    }
                );
            }
        }
        // Obsolete retries do not keep quiescence alive or emit a second drop.
        let count = e.get_trace_snapshot().total_emitted_ever;
        e.advance_until(VirtualTime(4_000_000_000), 1000).unwrap();
        assert_eq!(e.get_trace_snapshot().total_emitted_ever, count);
        assert_idle(&mut e);
        return e;
    }
    if label == "s3" {
        e = Engine::with_default_trace_capacity();
        e.create_host("host1".into(), vec!["eth0".into()]).unwrap();
        let config = HostIpv4Config::new(Ipv4InterfaceConfig::new(H, 24).unwrap(), None).unwrap();
        let m = MacAddress([2, 0, 0, 0, 1, 1]);
        e.configure_host_ipv4(ep("host1", "eth0"), m, config, true)
            .unwrap();
        let target = Ipv4Address([172, 16, 5, 99]);
        e.send_ping("host1".into(), target, 1, 1).unwrap();
        e.run_until_quiescent(100).unwrap();
        assert_eq!(e.now().0, 0);
        let trace = e.get_trace_snapshot();
        assert_eq!(trace.records.len(), 2);
        assert!(
            matches!(&trace.records[0].event,ObservableEvent::PacketDropped {pdu:DroppedPdu::Ipv4(p),reason:DropReason::NoRouteToHost,..} if p.src_ip==H && p.dst_ip==target && p.ttl==64)
        );
        assert!(
            matches!(&trace.records[1].event,ObservableEvent::PingFailed {host,target_ip,identifier:1,sequence:1,reason:PingFailureReason::DestinationNetworkUnreachable} if host.0=="host1" && *target_ip==target)
        );
        assert!(e.pings.is_empty());
        assert!(e.scheduler.is_empty());
        e.configure_host_ipv4(ep("host1", "eth0"), m, config, true)
            .unwrap();
        return e;
    }
    if label == "host-mixed" {
        let unknown = Ipv4Address([192, 168, 1, 99]);
        e.send_ping("h".into(), unknown, 3, 17).unwrap();
        let mut p = request(1);
        p.src_ip = unknown;
        p.dst_ip = H;
        inject(&mut e, ep("h", "p"), p, 0);
        e.run_until_quiescent(1000).unwrap();
        let ds = drops(&e);
        assert_eq!(ds.len(), 2);
        assert_eq!(
            ds.iter()
                .map(|d| match &d.2.payload {
                    Ipv4Payload::Icmp(c) => c.identifier,
                    _ => 0,
                })
                .collect::<Vec<_>>(),
            vec![3, 1]
        );
        assert_eq!(e.get_trace_snapshot().records.iter().filter(|r|matches!(&r.event,ObservableEvent::PingFailed {host,target_ip,identifier:3,reason:PingFailureReason::ArpTimeout,..} if host.0=="h" && *target_ip==unknown)).count(),1);
        assert!(errors(&e).is_empty());
        assert_idle(&mut e);
        return e;
    }
    if label == "renew-expire" {
        let mut p = request(8);
        p.dst_ip = Ipv4Address([10, 0, 0, 30]);
        inject(&mut e, a.clone(), p.clone(), 0);
        e.run_until_quiescent(1000).unwrap();
        assert_eq!(e.router_arp_entry(&b, p.dst_ip).unwrap(), Some(mac(4)));
        let count = e.get_trace_snapshot().total_emitted_ever;
        e.advance_until(VirtualTime(1_000_000_100), 1000).unwrap();
        assert_eq!(count, e.get_trace_snapshot().total_emitted_ever);
        e.advance_until(VirtualTime(60_000_002_200), 1000).unwrap();
        assert_eq!(e.router_arp_entry(&b, p.dst_ip).unwrap(), None);
        p.payload = Ipv4Payload::Icmp(IcmpPacket {
            kind: IcmpKind::EchoRequest,
            identifier: 9,
            sequence: 23,
        });
        let now = e.now().0;
        inject(&mut e, a.clone(), p.clone(), now);
        e.run_until_quiescent(1000).unwrap();
        assert_eq!(e.router_arp_entry(&b, p.dst_ip).unwrap(), Some(mac(4)));
        assert_eq!(e.get_trace_snapshot().records.iter().filter(|r|matches!(&r.event,ObservableEvent::ArpPacketQueued{at_device,target_ip,..} if at_device.0=="r" && *target_ip==p.dst_ip)).count(),2);
        assert!(
            !drops(&e)
                .iter()
                .any(|d| d.3 == DropReason::ArpResolutionFailed)
        );
        assert_idle(&mut e);
        return e;
    }
    if label == "selected-egress-down" {
        let mut p = request(13);
        p.dst_ip = Ipv4Address([198, 51, 100, 7]);
        e.add_router_static_route(
            &"r".into(),
            crate::ipv4::Ipv4Network::new(p.dst_ip, 24).unwrap(),
            Some(Ipv4Address([10, 0, 0, 30])),
            "b".into(),
            1,
        )
        .unwrap();
        e.add_router_static_route(
            &"r".into(),
            crate::ipv4::Ipv4Network::new(H, 0).unwrap(),
            Some(H),
            "a".into(),
            99,
        )
        .unwrap();
        inject(&mut e, a.clone(), p.clone(), 0);
        e.advance_until(VirtualTime(2200), 1000).unwrap();
        e.set_interface_admin_state(b.clone(), false).unwrap();
        e.run_until_quiescent(1000).unwrap();
        assert!(errors(&e).is_empty());
        let trace = e.get_trace_snapshot();
        let ds: Vec<_> = trace
            .records
            .iter()
            .filter(|r| {
                matches!(
                    &r.event,
                    ObservableEvent::PacketDropped {
                        pdu: DroppedPdu::Ethernet(EthernetFrame {
                            payload: EthernetPayload::Ipv4(_),
                            ..
                        }),
                        ..
                    }
                )
            })
            .collect();
        assert_eq!(ds.len(), 1);
        p.ttl = 63;
        assert!(
            matches!(&ds[0].event,ObservableEvent::PacketDropped{reason:DropReason::EgressInterfaceDown,pdu:DroppedPdu::Ethernet(EthernetFrame{payload:EthernetPayload::Ipv4(observed),..}),..} if observed==&p)
        );
        assert_eq!(ds[0].at.0, 2300);
        assert!(!trace.records.iter().any(|r| matches!(
            &r.event,
            ObservableEvent::FrameTransmitted {
                frame: EthernetFrame {
                    payload: EthernetPayload::Ipv4(_),
                    ..
                },
                ..
            }
        )));
        assert_idle(&mut e);
        return e;
    }
    let mut packets = vec![request(3)];
    if label == "transit-reply" {
        packets[0].payload = Ipv4Payload::Icmp(IcmpPacket {
            kind: IcmpKind::EchoReply,
            identifier: 3,
            sequence: 17,
        });
    }
    if label == "fifo" || label.starts_with("mixed-") {
        packets.push(request(1));
        packets.push(request(2));
    }
    if label.starts_with("mixed-") {
        let summary = OriginalDatagramSummary::from_ipv4(&packets[0]);
        packets[1].payload = Ipv4Payload::IcmpError(match label {
            "mixed-te" => IcmpErrorPacket::TimeExceeded { original: summary },
            "mixed-net" => IcmpErrorPacket::DestinationNetworkUnreachable { original: summary },
            _ => IcmpErrorPacket::DestinationHostUnreachable { original: summary },
        });
    }
    if label == "no-return" {
        packets[0].src_ip = Ipv4Address([198, 18, 0, 10]);
    }
    if label == "same-key" {
        packets[0].src_ip = TARGET;
    }
    if label == "return-arp-fail" {
        packets[0].src_ip = Ipv4Address([192, 168, 1, 99]);
    }
    if label == "static-next" {
        packets[0].dst_ip = Ipv4Address([198, 51, 100, 7]);
        e.add_router_static_route(
            &"r".into(),
            crate::ipv4::Ipv4Network::new(packets[0].dst_ip, 24).unwrap(),
            Some(Ipv4Address([10, 0, 0, 99])),
            "b".into(),
            9,
        )
        .unwrap();
    }
    if label == "return-static" {
        // A static next hop is a router, not a host impersonating a gateway.
        e.disconnect_link(&"l2".into()).unwrap();
        e.create_router(
            "returner".into(),
            vec![
                RouterPortConfig::new(
                    "a".into(),
                    mac(5),
                    Ipv4InterfaceConfig::new(Ipv4Address([10, 0, 0, 30]), 24).unwrap(),
                    true,
                ),
                RouterPortConfig::new(
                    "b".into(),
                    mac(6),
                    Ipv4InterfaceConfig::new(Ipv4Address([203, 0, 113, 1]), 24).unwrap(),
                    false,
                ),
            ],
        )
        .unwrap();
        e.connect_link("l-return".into(), b.clone(), ep("returner", "a"))
            .unwrap();
        e.add_router_static_route(
            &"r".into(),
            crate::ipv4::Ipv4Network::new(H, 32).unwrap(),
            Some(Ipv4Address([10, 0, 0, 30])),
            "b".into(),
            99,
        )
        .unwrap();
    }
    for p in &packets {
        inject(&mut e, a.clone(), p.clone(), 0);
    }
    if matches!(label, "return-down" | "egress-down" | "egress-reconnect") {
        e.advance_until(VirtualTime(100), 1000).unwrap();
        if label == "return-down" {
            e.set_interface_admin_state(a.clone(), false).unwrap();
        } else if label == "egress-down" {
            e.set_interface_admin_state(b.clone(), false).unwrap();
        } else {
            e.disconnect_link(&"l2".into()).unwrap();
            e.connect_link("l-new".into(), b.clone(), ep("peer", "p"))
                .unwrap();
        }
    }
    assert_eq!(
        e.run_until_quiescent(1000).unwrap().outcome,
        ExecutionOutcome::Quiescent
    );
    let ds = drops(&e);
    let originals: Vec<_> = ds.iter().filter(|d| !is_icmp_error(&d.2)).collect();
    let eligible = packets.iter().filter(|p| !is_icmp_error(p)).count();
    assert_eq!(originals.len(), eligible);
    // Arrival retained TTL64, queued/dropped datagrams TTL63: no second decrement.
    for p in &packets {
        assert!(e.get_trace_snapshot().records.iter().any(|r|matches!(&r.event,ObservableEvent::FrameDelivered {frame:EthernetFrame{payload:EthernetPayload::Ipv4(observed),..},..} if observed==p)));
    }
    for d in &ds {
        if d.0 == 3_000_000_100 {
            assert_eq!(d.2.ttl, 63);
            assert_eq!(d.3, DropReason::ArpResolutionFailed);
        }
    }
    let tx = errors(&e);
    if matches!(
        label,
        "no-return" | "return-down" | "same-key" | "return-arp-fail"
    ) {
        assert!(tx.is_empty());
        assert_eq!(
            ds.len(),
            if matches!(label, "same-key" | "return-arp-fail") {
                2
            } else {
                1
            }
        );
        if matches!(label, "same-key" | "return-arp-fail") {
            assert_eq!(ds[1].0, 6_000_000_200);
            assert_eq!(ds[1].3, DropReason::ArpResolutionFailed);
            assert!(matches!(
                &ds[1].2.payload,
                Ipv4Payload::IcmpError(IcmpErrorPacket::DestinationHostUnreachable { .. })
            ));
        }
    } else {
        assert_eq!(tx.len(), eligible);
        for (_, port, p) in &tx {
            assert_eq!(port, if label == "return-static" { &b } else { &a });
            assert_eq!(
                p.src_ip,
                if label == "return-static" {
                    Ipv4Address([10, 0, 0, 1])
                } else {
                    R
                }
            );
            assert_eq!(p.ttl, 64);
            let Ipv4Payload::IcmpError(error) = &p.payload else {
                unreachable!()
            };
            assert_eq!((error.icmp_type(), error.code()), (3, 1));
        }
    }
    assert!(!e.get_trace_snapshot().records.iter().any(|r| matches!(
        &r.event,
        ObservableEvent::PingCompleted { .. } | ObservableEvent::PingFailed { .. }
    )));
    assert_idle(&mut e);
    e
}

const CASES: &[&str] = &[
    "fifo",
    "transit-reply",
    "mixed-te",
    "mixed-net",
    "mixed-host",
    "no-return",
    "same-key",
    "return-arp-fail",
    "static-next",
    "return-static",
    "return-down",
    "egress-down",
    "egress-reconnect",
    "s3",
    "s7-no-gateway",
    "s7-bad-gateway",
    "host-mixed",
    "renew-expire",
    "selected-egress-down",
    "reply-host-exhaust",
    "reply-router-exhaust",
    "reply-host-down",
    "reply-router-down",
    "reply-host-link",
    "reply-router-link",
    "reply-host-reconnect",
    "reply-router-reconnect",
    "reply-host-flight",
    "reply-router-flight",
];
#[test]
fn failure_variations_have_normative_oracles() {
    for label in CASES {
        run_case(label);
    }
}

fn projection(e: &Engine, label: &str) -> Vec<String> {
    assert!(
        e.get_trace_snapshot()
            .records
            .windows(2)
            .all(|pair| pair[0].at <= pair[1].at && pair[0].seq.0 < pair[1].seq.0)
    );
    let mut rows = crate::icmp_router_tests::observation_projection(e, label)
        .into_iter()
        .map(|s| s.replacen("ICMP4|", "ARP5|", 1))
        .collect::<Vec<_>>();
    if label.starts_with("s7-") {
        rows.push(format!(
            "ARP5|{label}|sessions|h:{}|peer:{}",
            e.pings.keys().filter(|k| k.source.0 == "h").count(),
            e.pings.keys().filter(|k| k.source.0 == "peer").count()
        ));
    }
    for endpoint in if label == "s3" {
        vec![]
    } else {
        vec![ep("r", "a"), ep("r", "b")]
    } {
        for ip in [
            H,
            R,
            TARGET,
            Ipv4Address([10, 0, 0, 30]),
            Ipv4Address([192, 168, 1, 99]),
        ] {
            rows.push(format!(
                "ARP5|{label}|state|{}:{}|{:?}|{:?}",
                endpoint.device.0,
                endpoint.interface.0,
                ip.0,
                e.router_arp_entry(&endpoint, ip).unwrap().map(|m| m.0)
            ));
        }
    }
    rows
}
#[test]
fn arp_failure_projection_is_deterministic() {
    for label in CASES {
        let expected = projection(&run_case(label), label);
        for _ in 1..100 {
            assert_eq!(projection(&run_case(label), label), expected);
        }
        for row in expected {
            println!("{row}");
        }
    }
}
