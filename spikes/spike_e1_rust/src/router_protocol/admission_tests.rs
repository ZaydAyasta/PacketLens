use super::*;
use crate::ipv4::{HostIpv4Config, Ipv4InterfaceConfig};
use crate::router_engine::RouterPortConfig;

fn ip(a: u8, b: u8, c: u8, d: u8) -> Ipv4Address {
    Ipv4Address([a, b, c, d])
}
fn host_mac() -> MacAddress {
    MacAddress([2, 0, 0, 0, 1, 1])
}
fn router_mac() -> MacAddress {
    MacAddress([2, 0, 0, 0, 2, 1])
}
fn ingress() -> EndpointRef {
    EndpointRef::new("r1", "eth0")
}
fn fixture() -> Engine {
    let mut e = Engine::with_default_trace_capacity();
    e.create_host("host1".into(), vec!["eth0".into()]).unwrap();
    e.configure_host_ipv4(
        EndpointRef::new("host1", "eth0"),
        host_mac(),
        HostIpv4Config::new(
            Ipv4InterfaceConfig::new(ip(192, 168, 1, 10), 24).unwrap(),
            Some(ip(192, 168, 1, 1)),
        )
        .unwrap(),
        true,
    )
    .unwrap();
    e.create_router(
        "r1".into(),
        vec![
            RouterPortConfig::new(
                "eth0".into(),
                router_mac(),
                Ipv4InterfaceConfig::new(ip(192, 168, 1, 1), 24).unwrap(),
                true,
            ),
            RouterPortConfig::new(
                "eth1".into(),
                MacAddress([2, 0, 0, 0, 2, 2]),
                Ipv4InterfaceConfig::new(ip(203, 0, 113, 1), 24).unwrap(),
                false,
            ),
        ],
    )
    .unwrap();
    e.connect_link("l1".into(), EndpointRef::new("host1", "eth0"), ingress())
        .unwrap();
    e
}
fn request() -> Ipv4Packet {
    Ipv4Packet {
        src_ip: ip(192, 168, 1, 10),
        dst_ip: ip(198, 51, 100, 1),
        ttl: 64,
        payload: Ipv4Payload::Icmp(IcmpPacket {
            kind: IcmpKind::EchoRequest,
            identifier: 7,
            sequence: 1,
        }),
    }
}
fn frame(packet: Ipv4Packet, destination: MacAddress) -> EthernetFrame {
    EthernetFrame {
        src_mac: host_mac(),
        dst_mac: destination,
        payload: EthernetPayload::Ipv4(packet),
    }
}
fn inject(e: &mut Engine, f: EthernetFrame, at: u64) {
    let dst = ingress();
    let link_instance = e.interface(&dst).unwrap().attached_link.unwrap();
    e.schedule(
        at,
        InternalEvent::FrameArrived {
            link_instance,
            dst,
            frame: f,
        },
    )
    .unwrap();
}
fn assert_single_ipv4_drop(e: &Engine, original: &Ipv4Packet, reason: &str) {
    let t = e.get_trace_snapshot();
    assert_eq!(t.records.len(), 2);
    assert!(
        matches!(&t.records[0].event,ObservableEvent::FrameDelivered {at_endpoint,..} if *at_endpoint==ingress())
    );
    let mut received = original.clone();
    if reason == "NoRouteToHost" {
        received.ttl += 1;
    }
    assert!(
        matches!(&t.records[0].event,ObservableEvent::FrameDelivered {frame:EthernetFrame {payload:EthernetPayload::Ipv4(p),..},..} if *p==received)
    );
    match &t.records[1].event {
        ObservableEvent::PacketDropped {
            at_device,
            at_interface,
            pdu: DroppedPdu::Ipv4(p),
            reason: r,
        } => {
            assert_eq!(at_device.0, "r1");
            assert_eq!(at_interface.as_ref().unwrap().0, "eth0");
            assert_eq!(p, original);
            assert_eq!(format!("{r:?}"), reason);
        }
        other => panic!("Expected IPv4 drop, got {other:?}"),
    }
    assert_eq!((t.records[0].seq.0, t.records[1].seq.0), (1, 2));
    assert_eq!((t.records[0].at.0, t.records[1].at.0, e.now().0), (0, 0, 0));
    assert_eq!((t.total_emitted_ever, t.dropped_by_capacity), (2, 0));
}
#[test]
fn multicast_l2_unicast_ip_is_ipv4_policy_drop() {
    for destination in [MacAddress([1, 0, 94, 0, 0, 1]), MacAddress::BROADCAST] {
        let mut e = s8_fixture();
        let p = request();
        inject(&mut e, frame(p.clone(), destination), 0);
        assert_eq!(
            e.run_until_quiescent(1000).unwrap().outcome,
            ExecutionOutcome::Quiescent
        );
        assert_single_ipv4_drop(&e, &p, "UnsupportedIpDestination");
    }
}

#[test]
fn s8c_ip_broadcast_multicast_is_rejected_before_ttl_and_local_service() {
    for destination in [
        ip(255, 255, 255, 255),
        ip(192, 168, 1, 255),
        ip(203, 0, 113, 255),
        ip(224, 0, 0, 1),
        ip(239, 255, 255, 255),
    ] {
        for ttl in [0, 1, 64] {
            let mut e = s8_fixture();
            let mut p = request();
            p.dst_ip = destination;
            p.ttl = ttl;
            inject(&mut e, frame(p.clone(), router_mac()), 0);
            e.run_until_quiescent(1000).unwrap();
            assert_single_ipv4_drop(&e, &p, "UnsupportedIpDestination");
        }
    }
}
#[test]
fn s8d_invalid_source_is_rejected_before_echo_and_transit() {
    for source in [
        ip(0, 0, 0, 0),
        ip(0, 1, 2, 3),
        ip(127, 0, 0, 1),
        ip(224, 0, 0, 1),
        ip(255, 255, 255, 255),
        ip(192, 168, 1, 255),
        ip(203, 0, 113, 255),
        ip(0, 255, 255, 255),
        ip(127, 255, 255, 254),
        ip(239, 255, 255, 255),
    ] {
        for destination in [ip(198, 51, 100, 1), ip(192, 168, 1, 1), ip(203, 0, 113, 1)] {
            for ttl in [0, 1, 64] {
                let mut e = s8_fixture();
                let mut p = request();
                p.src_ip = source;
                p.dst_ip = destination;
                p.ttl = ttl;
                inject(&mut e, frame(p.clone(), router_mac()), 0);
                e.run_until_quiescent(1000).unwrap();
                assert_single_ipv4_drop(&e, &p, "InvalidIpv4Source");
            }
        }
    }
}

fn seed(e: &mut Engine) {
    assert_eq!(
        (e.now().0, e.get_trace_snapshot().total_emitted_ever),
        (0, 0)
    );
    let endpoint = ingress();
    let address = ip(192, 168, 1, 10);
    let epoch = e.next_epoch_value().unwrap();
    e.router_arp_mut(&endpoint).unwrap().cache.insert(
        address,
        ArpEntry {
            mac: host_mac(),
            epoch,
        },
    );
    e.schedule(
        ARP_CACHE_TTL_NS,
        InternalEvent::RouterArpExpireTimer {
            endpoint,
            ip: address,
            entry_epoch: epoch,
        },
    )
    .unwrap();
}
fn s8_fixture() -> Engine {
    let mut e = fixture();
    seed(&mut e);
    e
}
fn error(kind: u8) -> IcmpErrorPacket {
    let original = OriginalDatagramSummary::from_ipv4(&request());
    match kind {
        0 => IcmpErrorPacket::TimeExceeded { original },
        1 => IcmpErrorPacket::DestinationNetworkUnreachable { original },
        2 => IcmpErrorPacket::DestinationHostUnreachable { original },
        _ => panic!("unknown fixture kind"),
    }
}
#[test]
fn received_host_unreachable_has_code_one() {
    let message = error(2);
    assert_eq!((message.icmp_type(), message.code()), (3, 1));
    assert_eq!(
        message.original(),
        &OriginalDatagramSummary::from_ipv4(&request())
    );
    let mut p = request();
    p.payload = Ipv4Payload::IcmpError(message);
    let summary = OriginalDatagramSummary::from_ipv4(&p);
    assert_eq!(
        (summary.echo_identifier, summary.echo_sequence),
        (None, None)
    );
}
#[test]
fn s8a_eligible_generates_exactly_one_error_and_real_delivery_without_ping() {
    for ttl in [0, 1, 64] {
        let mut e = s8_fixture();
        let mut p = request();
        p.ttl = ttl;
        inject(&mut e, frame(p.clone(), router_mac()), 0);
        assert_eq!(
            e.run_until_quiescent(1000).unwrap().outcome,
            ExecutionOutcome::Quiescent
        );
        let t = e.get_trace_snapshot();
        assert_eq!(t.records.len(), 4);
        assert_eq!(e.now().0, 1100);
        let mut dropped = p.clone();
        if ttl > 1 {
            dropped.ttl -= 1;
        }
        assert!(
            matches!(&t.records[0].event,ObservableEvent::FrameDelivered {frame:f,..} if *f==frame(p.clone(),router_mac()))
        );
        assert!(
            matches!(&t.records[1].event,ObservableEvent::PacketDropped {at_device,at_interface,pdu:DroppedPdu::Ipv4(d),reason} if at_device.0=="r1" && at_interface==&Some("eth0".into()) && *d==dropped && *reason==if ttl>1 {DropReason::NoRouteToHost} else {DropReason::TtlExpiredInTransit})
        );
        let response = Ipv4Packet {
            src_ip: ip(192, 168, 1, 1),
            dst_ip: p.src_ip,
            ttl: 64,
            payload: Ipv4Payload::IcmpError(if ttl > 1 { error(1) } else { error(0) }),
        };
        let expected = EthernetFrame {
            src_mac: router_mac(),
            dst_mac: host_mac(),
            payload: EthernetPayload::Ipv4(response),
        };
        assert!(
            matches!(&t.records[2].event,ObservableEvent::FrameTransmitted {from,to,frame,..} if *from==ingress() && *to==EndpointRef::new("host1","eth0") && *frame==expected)
        );
        assert!(
            matches!(&t.records[3].event,ObservableEvent::FrameDelivered {at_endpoint,frame,..} if *at_endpoint==EndpointRef::new("host1","eth0") && *frame==expected)
        );
        assert_eq!(
            t.records.iter().map(|r| r.seq.0).collect::<Vec<_>>(),
            vec![1, 2, 3, 4]
        );
        assert_eq!(
            t.records.iter().map(|r| r.at.0).collect::<Vec<_>>(),
            vec![0, 0, 100, 1100]
        );
    }
}
#[test]
fn s8b_all_error_inputs_ttl_zero_one_sixty_four_never_recurse() {
    for kind in 0..3 {
        for ttl in [0, 1, 64] {
            let mut e = s8_fixture();
            let mut p = request();
            p.payload = Ipv4Payload::IcmpError(error(kind));
            p.ttl = ttl;
            inject(&mut e, frame(p.clone(), router_mac()), 0);
            e.run_until_quiescent(1000).unwrap();
            let mut expected = p;
            if ttl > 1 {
                expected.ttl -= 1;
            }
            assert_single_ipv4_drop(
                &e,
                &expected,
                if ttl > 1 {
                    "NoRouteToHost"
                } else {
                    "TtlExpiredInTransit"
                },
            );
        }
    }
}
#[test]
fn preseed_has_normal_epoch_expiry_without_fake_learning() {
    let mut e = s8_fixture();
    assert!(e.get_trace_snapshot().records.is_empty());
    e.advance_until(VirtualTime(ARP_CACHE_TTL_NS - 1), 1000)
        .unwrap();
    assert_eq!(
        e.router_arp_entry(&ingress(), ip(192, 168, 1, 10)).unwrap(),
        Some(host_mac())
    );
    e.advance_until(VirtualTime(ARP_CACHE_TTL_NS), 1000)
        .unwrap();
    assert_eq!(
        e.router_arp_entry(&ingress(), ip(192, 168, 1, 10)).unwrap(),
        None
    );
    let t = e.get_trace_snapshot();
    assert_eq!(t.records.len(), 1);
    assert!(
        matches!(&t.records[0].event,ObservableEvent::TableUpdated {at_device,update:TableUpdateDetail::ArpEntryExpired {ip:address}} if at_device.0=="r1" && *address==ip(192,168,1,10))
    );
    assert_eq!(t.records[0].observation_interface, Some("eth0".into()));
}

#[test]
fn foreign_unicast_mac_is_an_ethernet_drop_even_for_forbidden_ip() {
    for destination in [ip(198, 51, 100, 1), ip(224, 0, 0, 1), ip(192, 168, 1, 1)] {
        let mut e = s8_fixture();
        let mut p = request();
        p.dst_ip = destination;
        let input = frame(p, MacAddress([2, 0, 0, 0, 99, 1]));
        inject(&mut e, input.clone(), 0);
        e.run_until_quiescent(1000).unwrap();
        let t = e.get_trace_snapshot();
        assert_eq!(t.records.len(), 2);
        assert!(
            matches!(&t.records[1].event,ObservableEvent::PacketDropped {reason:DropReason::DestinationMacMismatch,pdu:DroppedPdu::Ethernet(f),..} if *f==input)
        );
        assert_eq!(e.now().0, 0);
    }
}
#[test]
fn valid_local_echo_before_ttl_zero_one_sixty_four() {
    for dst in [ip(192, 168, 1, 1), ip(203, 0, 113, 1)] {
        for ttl in [0, 1, 64] {
            let mut e = s8_fixture();
            let mut p = request();
            p.dst_ip = dst;
            p.ttl = ttl;
            inject(&mut e, frame(p, router_mac()), 0);
            e.run_until_quiescent(1000).unwrap();
            let t = e.get_trace_snapshot();
            assert_eq!(t.records.len(), 3);
            assert!(
                matches!(&t.records[1].event,ObservableEvent::FrameTransmitted {frame:EthernetFrame {payload:EthernetPayload::Ipv4(p),..},..} if p.src_ip==dst && p.dst_ip==ip(192,168,1,10) && p.ttl==64 && matches!(&p.payload,Ipv4Payload::Icmp(c) if c.kind==IcmpKind::EchoReply && c.identifier==7 && c.sequence==1))
            );
            assert_eq!(e.now().0, 1100);
        }
    }
}

fn arp_transit_fixture() -> Engine {
    let mut e = fixture();
    e.set_interface_admin_state(EndpointRef::new("r1", "eth1"), true)
        .unwrap();
    e.create_router(
        "r2".into(),
        vec![
            RouterPortConfig::new(
                "eth0".into(),
                MacAddress([2, 0, 0, 0, 3, 1]),
                Ipv4InterfaceConfig::new(ip(203, 0, 113, 2), 24).unwrap(),
                true,
            ),
            RouterPortConfig::new(
                "eth1".into(),
                MacAddress([2, 0, 0, 0, 3, 2]),
                Ipv4InterfaceConfig::new(ip(10, 0, 0, 1), 24).unwrap(),
                false,
            ),
        ],
    )
    .unwrap();
    e.connect_link(
        "l2".into(),
        EndpointRef::new("r1", "eth1"),
        EndpointRef::new("r2", "eth0"),
    )
    .unwrap();
    e.add_router_static_route(
        &"r1".into(),
        crate::ipv4::Ipv4Network::new(ip(198, 51, 100, 0), 24).unwrap(),
        Some(ip(203, 0, 113, 2)),
        "eth1".into(),
        10,
    )
    .unwrap();
    e.add_router_static_route(
        &"r2".into(),
        crate::ipv4::Ipv4Network::new(ip(192, 168, 1, 0), 24).unwrap(),
        Some(ip(203, 0, 113, 1)),
        "eth0".into(),
        10,
    )
    .unwrap();
    e
}
fn post_arp_case(kind: u8, ttl: u8, fail: bool) -> Engine {
    let mut e = arp_transit_fixture();
    if fail {
        e.set_interface_admin_state(EndpointRef::new("r2", "eth0"), false)
            .unwrap();
    }
    let mut p = request();
    p.ttl = ttl;
    p.payload = Ipv4Payload::IcmpError(error(kind));
    let input = frame(p.clone(), router_mac());
    inject(&mut e, input.clone(), 0);
    e.step_events(1).unwrap();
    let pending = &e
        .router_arp_mut(&EndpointRef::new("r1", "eth1"))
        .unwrap()
        .pending[&ip(203, 0, 113, 2)][0];
    assert_eq!(pending._received, Some((ingress(), input)));
    let mut forwarded = p;
    forwarded.ttl -= 1;
    assert_eq!(pending.packet, forwarded);
    assert_eq!(
        e.run_until_quiescent(1000).unwrap().outcome,
        ExecutionOutcome::Quiescent
    );
    let t = e.get_trace_snapshot();
    let drops: Vec<_> = t
        .records
        .iter()
        .filter_map(|r| {
            if let ObservableEvent::PacketDropped {
                pdu: DroppedPdu::Ipv4(p),
                reason,
                ..
            } = &r.event
            {
                Some((r.at.0, p, reason))
            } else {
                None
            }
        })
        .collect();
    assert_eq!(drops.len(), 1);
    let tx: Vec<_> = t
        .records
        .iter()
        .filter_map(|r| {
            if let ObservableEvent::FrameTransmitted {
                frame:
                    EthernetFrame {
                        payload: EthernetPayload::Ipv4(p),
                        ..
                    },
                ..
            } = &r.event
            {
                Some(p)
            } else {
                None
            }
        })
        .collect();
    if fail {
        assert!(tx.is_empty());
        assert_eq!(
            drops[0],
            (3_000_000_100, &forwarded, &DropReason::ArpResolutionFailed)
        );
    } else {
        assert_eq!(tx, vec![&forwarded]);
        let mut discarded = forwarded;
        if ttl > 2 {
            discarded.ttl -= 1;
        }
        assert_eq!(
            drops[0],
            (
                3300,
                &discarded,
                if ttl == 2 {
                    &DropReason::TtlExpiredInTransit
                } else {
                    &DropReason::NoRouteToHost
                }
            )
        );
    }
    assert!(!t.records.iter().any(|r| matches!(
        r.event,
        ObservableEvent::PingCompleted { .. } | ObservableEvent::PingFailed { .. }
    )));
    e
}
#[test]
fn original_error_and_context_survive_arp_and_never_generate_secondary_error() {
    for kind in 0..3 {
        for ttl in [2, 64] {
            post_arp_case(kind, ttl, false);
        }
        post_arp_case(kind, 64, true);
    }
}
#[test]
fn no_return_and_suppression_do_not_depend_on_neighbor_cache() {
    for warm in [false, true] {
        for ttl in [0, 1, 64] {
            for kind in 0..4 {
                let mut e = if warm { s8_fixture() } else { fixture() };
                let mut p = request();
                p.ttl = ttl;
                p.src_ip = ip(198, 18, 0, 10);
                if kind < 3 {
                    p.payload = Ipv4Payload::IcmpError(error(kind));
                }
                inject(&mut e, frame(p.clone(), router_mac()), 0);
                e.run_until_quiescent(1000).unwrap();
                if ttl > 1 {
                    p.ttl -= 1;
                }
                assert_single_ipv4_drop(
                    &e,
                    &p,
                    if ttl > 1 {
                        "NoRouteToHost"
                    } else {
                        "TtlExpiredInTransit"
                    },
                );
            }
        }
    }
}
#[test]
fn remote_dot255_is_not_invented_as_a_broadcast_and_multicast_bounds_are_exact() {
    for dst in [
        ip(198, 51, 100, 255),
        ip(223, 255, 255, 255),
        ip(240, 0, 0, 1),
    ] {
        let mut e = s8_fixture();
        let mut p = request();
        p.dst_ip = dst;
        inject(&mut e, frame(p, router_mac()), 0);
        e.run_until_quiescent(1000).unwrap();
        let t = e.get_trace_snapshot();
        assert_eq!(t.records.len(), 4);
        assert!(matches!(
            &t.records[1].event,
            ObservableEvent::PacketDropped {
                reason: DropReason::NoRouteToHost,
                ..
            }
        ));
    }
}
#[test]
fn slash31_and_slash32_addresses_are_not_directed_broadcasts() {
    for prefix in [31, 32] {
        let mut e = Engine::with_default_trace_capacity();
        e.create_host("host1".into(), vec!["eth0".into()]).unwrap();
        if prefix == 31 {
            e.set_interface_ipv4_mode(
                EndpointRef::new("host1", "eth0"),
                crate::ipv4::Ipv4LinkMode::PointToPoint,
            )
            .unwrap();
        }
        e.configure_host_ipv4(
            EndpointRef::new("host1", "eth0"),
            host_mac(),
            HostIpv4Config::new(
                Ipv4InterfaceConfig::new(ip(172, 16, 0, 0), prefix).unwrap(),
                None,
            )
            .unwrap(),
            true,
        )
        .unwrap();
        let mut port = RouterPortConfig::new(
            "eth0".into(),
            router_mac(),
            Ipv4InterfaceConfig::new(ip(172, 16, 0, 1), prefix).unwrap(),
            true,
        );
        if prefix == 31 {
            port.mode = crate::ipv4::Ipv4LinkMode::PointToPoint;
        }
        e.create_router(
            "r1".into(),
            vec![
                port,
                RouterPortConfig::new(
                    "eth1".into(),
                    MacAddress([2, 0, 0, 0, 2, 2]),
                    Ipv4InterfaceConfig::new(ip(203, 0, 113, 1), 24).unwrap(),
                    false,
                ),
            ],
        )
        .unwrap();
        e.connect_link("l1".into(), EndpointRef::new("host1", "eth0"), ingress())
            .unwrap();
        if prefix == 32 {
            e.add_router_static_route(
                &"r1".into(),
                crate::ipv4::Ipv4Network::new(ip(172, 16, 0, 0), 32).unwrap(),
                None,
                "eth0".into(),
                1,
            )
            .unwrap();
        }
        e.validate_ipv4_topology().unwrap();
        let mut p = request();
        p.src_ip = ip(172, 16, 0, 0);
        p.dst_ip = ip(172, 16, 0, 1);
        p.ttl = 0;
        inject(&mut e, frame(p, router_mac()), 0);
        e.run_until_quiescent(1000).unwrap();
        let t = e.get_trace_snapshot();
        assert!(
            !t.records
                .iter()
                .any(|r| matches!(r.event, ObservableEvent::PacketDropped { .. }))
        );
        assert_eq!(t.records.iter().filter(|r|matches!(&r.event,ObservableEvent::FrameTransmitted {frame:EthernetFrame {payload:EthernetPayload::Ipv4(p),..},..} if p.src_ip==ip(172,16,0,1) && p.dst_ip==ip(172,16,0,0) && p.ttl==64 && matches!(&p.payload,Ipv4Payload::Icmp(c) if c.kind==IcmpKind::EchoReply))).count(),1);
    }
}
#[test]
fn failed_ingress_is_not_reported_as_ipv4_admission() {
    for stale in [false, true] {
        let mut e = s8_fixture();
        let input = frame(request(), router_mac());
        inject(&mut e, input.clone(), 1000);
        if stale {
            e.disconnect_link(&"l1".into()).unwrap();
            e.connect_link(
                "replacement".into(),
                EndpointRef::new("host1", "eth0"),
                ingress(),
            )
            .unwrap();
        } else {
            e.set_interface_admin_state(ingress(), false).unwrap();
        }
        e.run_until_quiescent(1000).unwrap();
        let t = e.get_trace_snapshot();
        assert_eq!(t.records.len(), 1);
        assert!(
            matches!(&t.records[0].event,ObservableEvent::PacketDropped {pdu:DroppedPdu::Ethernet(f),reason,..} if *f==input && *reason==if stale {DropReason::LinkDisconnectedInFlight}else{DropReason::IngressInterfaceDown})
        );
        assert_eq!(e.now().0, 1000);
    }
}
#[test]
fn seed_renewal_invalidates_old_epoch_without_ipv4_learning() {
    let mut e = s8_fixture();
    e.advance_until(VirtualTime(10), 1000).unwrap();
    let arp = EthernetFrame {
        src_mac: host_mac(),
        dst_mac: router_mac(),
        payload: EthernetPayload::Arp(ArpPacket {
            operation: ArpOperation::Reply,
            sender_mac: host_mac(),
            sender_ip: ip(192, 168, 1, 10),
            target_mac: router_mac(),
            target_ip: ip(192, 168, 1, 1),
        }),
    };
    inject(&mut e, arp, 10);
    e.run_until_quiescent(1000).unwrap();
    let before = e.get_trace_snapshot();
    e.advance_until(VirtualTime(ARP_CACHE_TTL_NS), 1000)
        .unwrap();
    assert_eq!(e.get_trace_snapshot(), before);
    assert_eq!(
        e.router_arp_entry(&ingress(), ip(192, 168, 1, 10)).unwrap(),
        Some(host_mac())
    );
    e.advance_until(VirtualTime(ARP_CACHE_TTL_NS + 10), 1000)
        .unwrap();
    assert_eq!(
        e.router_arp_entry(&ingress(), ip(192, 168, 1, 10)).unwrap(),
        None
    );
    assert_eq!(e.get_trace_snapshot().records.len(), 3);
}
#[test]
fn legacy_error_tags_cannot_escape_suppression_or_recurse_after_arp() {
    for kind in [IcmpKind::TimeExceeded, IcmpKind::DestinationHostUnreachable] {
        for pending in [false, true] {
            let mut e = if pending {
                arp_transit_fixture()
            } else {
                s8_fixture()
            };
            if pending {
                e.set_interface_admin_state(EndpointRef::new("r2", "eth0"), false)
                    .unwrap();
            }
            let mut p = request();
            p.payload = Ipv4Payload::Icmp(IcmpPacket {
                kind,
                identifier: 7,
                sequence: 1,
            });
            inject(&mut e, frame(p.clone(), router_mac()), 0);
            e.run_until_quiescent(1000).unwrap();
            p.ttl -= 1;
            if pending {
                let t = e.get_trace_snapshot();
                assert!(!t.records.iter().any(|r| matches!(
                    &r.event,
                    ObservableEvent::FrameTransmitted {
                        frame: EthernetFrame {
                            payload: EthernetPayload::Ipv4(_),
                            ..
                        },
                        ..
                    }
                )));
                assert_eq!(t.records.iter().filter(|r|matches!(&r.event,ObservableEvent::PacketDropped {pdu:DroppedPdu::Ipv4(d),reason:DropReason::ArpResolutionFailed,..} if *d==p)).count(),1);
            } else {
                assert_single_ipv4_drop(&e, &p, "NoRouteToHost");
            }
        }
    }
}
#[test]
fn nearest_valid_source_prefixes_are_not_rejected_or_learned_from_ipv4() {
    for source in [
        ip(1, 0, 0, 1),
        ip(126, 255, 255, 254),
        ip(128, 0, 0, 1),
        ip(223, 255, 255, 254),
    ] {
        let mut e = s8_fixture();
        let mut p = request();
        p.src_ip = source;
        inject(&mut e, frame(p.clone(), router_mac()), 0);
        e.run_until_quiescent(1000).unwrap();
        p.ttl = 63;
        assert_single_ipv4_drop(&e, &p, "NoRouteToHost");
        assert_eq!(e.router_arp_entry(&ingress(), source).unwrap(), None);
    }
}

fn cases() -> Vec<String> {
    let mut labels = Vec::new();
    for ttl in [0, 1, 64] {
        labels.push(format!("A:{ttl}"));
        for kind in 0..3 {
            for missing in 0..2 {
                labels.push(format!("B:{kind}:{ttl}:{missing}"));
            }
        }
        for dst in 0..5 {
            labels.push(format!("C:{dst}:{ttl}"));
        }
        for group in 0..2 {
            labels.push(format!("L:{group}:{ttl}"));
        }
        for src in 0..10 {
            for dst in 0..3 {
                for route in 0..2 {
                    labels.push(format!("D:{src}:{dst}:{ttl}:{route}"));
                }
            }
        }
        for warm in 0..2 {
            labels.push(format!("N:{ttl}:{warm}"));
        }
        for dst in 0..2 {
            labels.push(format!("E:{dst}:{ttl}"));
        }
    }
    for kind in 0..3 {
        for ttl in [2, 64] {
            labels.push(format!("P:{kind}:{ttl}:0"));
        }
        labels.push(format!("P:{kind}:64:1"));
    }
    labels.extend(["O:0".into(), "O:1".into()]);
    labels
}
fn ordered_case(reverse: bool) -> Engine {
    let mut e = s8_fixture();
    let mut inputs = Vec::new();
    for kind in 0..3 {
        let mut p = request();
        if kind < 2 {
            p.payload = Ipv4Payload::IcmpError(error(kind));
        } else {
            p.src_ip = ip(0, 1, 2, 3);
        }
        inputs.push(p);
    }
    if reverse {
        inputs.reverse();
    }
    for p in &inputs {
        inject(&mut e, frame(p.clone(), router_mac()), 0);
    }
    e.run_until_quiescent(1000).unwrap();
    let t = e.get_trace_snapshot();
    assert_eq!(t.records.len(), 6);
    for (index, p) in inputs.into_iter().enumerate() {
        assert!(
            matches!(&t.records[index*2].event,ObservableEvent::FrameDelivered {frame:f,..} if *f==frame(p.clone(),router_mac()))
        );
        let invalid = p.src_ip == ip(0, 1, 2, 3);
        let mut dropped = p;
        if !invalid {
            dropped.ttl -= 1;
        }
        assert!(
            matches!(&t.records[index*2+1].event,ObservableEvent::PacketDropped {pdu:DroppedPdu::Ipv4(p),reason,..} if *p==dropped && *reason==if invalid {DropReason::InvalidIpv4Source}else{DropReason::NoRouteToHost})
        );
    }
    assert_eq!(
        t.records.iter().map(|r| r.seq.0).collect::<Vec<_>>(),
        vec![1, 2, 3, 4, 5, 6]
    );
    e
}
fn execute_case(label: &str) -> Engine {
    let parts: Vec<_> = label.split(':').collect();
    let n = |index: usize| parts[index].parse::<u8>().unwrap();
    if parts[0] == "P" {
        return post_arp_case(n(1), n(2), n(3) == 1);
    }
    if parts[0] == "O" {
        return ordered_case(n(1) == 1);
    }
    let mut e = if parts[0] == "N" && n(2) == 0 {
        fixture()
    } else {
        s8_fixture()
    };
    let mut p = request();
    let mut mac = router_mac();
    let mut expected = None;
    match parts[0] {
        "A" => {
            p.ttl = n(1);
        }
        "B" => {
            p.payload = Ipv4Payload::IcmpError(error(n(1)));
            p.ttl = n(2);
            if n(3) == 1 {
                p.src_ip = ip(198, 18, 0, 10);
            }
            expected = Some(if p.ttl <= 1 {
                "TtlExpiredInTransit"
            } else {
                "NoRouteToHost"
            });
        }
        "C" => {
            p.dst_ip = [
                ip(255, 255, 255, 255),
                ip(192, 168, 1, 255),
                ip(203, 0, 113, 255),
                ip(224, 0, 0, 1),
                ip(239, 255, 255, 255),
            ][n(1) as usize];
            p.ttl = n(2);
            expected = Some("UnsupportedIpDestination");
        }
        "L" => {
            mac = if n(1) == 0 {
                MacAddress::BROADCAST
            } else {
                MacAddress([1, 0, 94, 0, 0, 1])
            };
            p.ttl = n(2);
            expected = Some("UnsupportedIpDestination");
        }
        "D" => {
            p.src_ip = [
                ip(0, 0, 0, 0),
                ip(0, 1, 2, 3),
                ip(127, 0, 0, 1),
                ip(224, 0, 0, 1),
                ip(255, 255, 255, 255),
                ip(192, 168, 1, 255),
                ip(203, 0, 113, 255),
                ip(0, 255, 255, 255),
                ip(127, 255, 255, 254),
                ip(239, 255, 255, 255),
            ][n(1) as usize];
            p.dst_ip = [ip(198, 51, 100, 1), ip(192, 168, 1, 1), ip(203, 0, 113, 1)][n(2) as usize];
            p.ttl = n(3);
            if n(4) == 1 {
                e.add_router_static_route(
                    &"r1".into(),
                    crate::ipv4::Ipv4Network::new(ip(0, 0, 0, 0), 0).unwrap(),
                    None,
                    "eth0".into(),
                    10,
                )
                .unwrap();
            }
            expected = Some("InvalidIpv4Source");
        }
        "N" => {
            p.src_ip = ip(198, 18, 0, 10);
            p.ttl = n(1);
            expected = Some(if p.ttl <= 1 {
                "TtlExpiredInTransit"
            } else {
                "NoRouteToHost"
            });
        }
        "E" => {
            p.dst_ip = if n(1) == 0 {
                ip(192, 168, 1, 1)
            } else {
                ip(203, 0, 113, 1)
            };
            p.ttl = n(2);
        }
        _ => panic!("unknown case"),
    }
    inject(&mut e, frame(p.clone(), mac), 0);
    assert_eq!(
        e.run_until_quiescent(1000).unwrap().outcome,
        ExecutionOutcome::Quiescent
    );
    if let Some(reason) = expected {
        if matches!(reason, "NoRouteToHost") {
            p.ttl -= 1;
        }
        assert_single_ipv4_drop(&e, &p, reason);
    } else {
        assert_eq!(
            e.get_trace_snapshot().records.len(),
            if parts[0] == "A" { 4 } else { 3 }
        );
    }
    // A configuration mutation is a black-box assertion that no Ping session was invented.
    e.add_router_static_route(
        &"r1".into(),
        crate::ipv4::Ipv4Network::new(ip(10, 123, 0, 1), 32).unwrap(),
        None,
        "eth0".into(),
        1,
    )
    .unwrap();
    e
}
fn projection(e: &Engine, label: &str) -> Vec<String> {
    let mut rows = crate::icmp_router_tests::observation_projection(e, label)
        .into_iter()
        .map(|r| r.replacen("ICMP4|", "ADM5|", 1))
        .collect::<Vec<_>>();
    let mac = |m: Option<MacAddress>| {
        m.map(|m| {
            m.0.iter()
                .map(|b| format!("{b:02x}"))
                .collect::<Vec<_>>()
                .join(":")
        })
        .unwrap_or("-".into())
    };
    rows.push(format!(
        "ADM5|{label}|state|{}|{}|{}",
        mac(e.arp_entry(&"host1".into(), ip(192, 168, 1, 1))),
        mac(e.router_arp_entry(&ingress(), ip(192, 168, 1, 10)).unwrap()),
        mac(e
            .router_arp_entry(&EndpointRef::new("r1", "eth1"), ip(203, 0, 113, 2))
            .unwrap())
    ));
    if label.starts_with("P:") {
        rows.push(format!(
            "ADM5|{label}|state-r2|{}",
            mac(e
                .router_arp_entry(&EndpointRef::new("r2", "eth0"), ip(203, 0, 113, 1))
                .unwrap())
        ));
    }
    rows
}
#[test]
fn s8_matrix_is_deterministic_across_one_hundred_reconstructions_and_order_variants() {
    for label in cases() {
        let expected = projection(&execute_case(&label), &label);
        for _ in 1..100 {
            assert_eq!(
                projection(&execute_case(&label), &label),
                expected,
                "{label}"
            );
        }
        for row in expected {
            println!("{row}");
        }
    }
}
