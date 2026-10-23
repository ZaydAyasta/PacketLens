use spike_e1_rust::InterfaceName;
use spike_e1_rust::Ipv4Address;
use spike_e1_rust::ipv4::ConfigurationError;
use spike_e1_rust::ipv4::Ipv4PrefixLength;
use spike_e1_rust::ipv4::{HostIpv4Config, Ipv4InterfaceConfig, Ipv4Network};
use spike_e1_rust::routing::RouterConfiguration;

fn ip(a: u8, b: u8, c: u8, d: u8) -> Ipv4Address {
    Ipv4Address([a, b, c, d])
}
fn interface(name: &str) -> InterfaceName {
    InterfaceName::from(name)
}
fn router() -> RouterConfiguration {
    RouterConfiguration::new(vec![
        (
            interface("eth0"),
            Ipv4InterfaceConfig::new(ip(192, 168, 1, 1), 24).unwrap(),
        ),
        (
            interface("eth1"),
            Ipv4InterfaceConfig::new(ip(172, 16, 1, 1), 24).unwrap(),
        ),
    ])
    .unwrap()
}

#[test]
fn rt_02_normalized_duplicates_are_rejected_atomically() {
    let mut router = router();
    let prefix = Ipv4Network::new(ip(10, 1, 1, 99), 24).unwrap();
    router
        .add_static_route(prefix, Some(ip(172, 16, 1, 2)), interface("eth1"), 1)
        .unwrap();
    let before = router.clone();
    assert!(
        router
            .add_static_route(
                Ipv4Network::new(ip(10, 1, 1, 0), 24).unwrap(),
                Some(ip(172, 16, 1, 3)),
                interface("eth1"),
                999
            )
            .is_err()
    );
    assert_eq!(router, before);
    assert!(
        router
            .add_static_route(
                Ipv4Network::new(ip(192, 168, 1, 77), 24).unwrap(),
                None,
                interface("eth0"),
                1
            )
            .is_err()
    );
    assert_eq!(router, before);
}

#[test]
fn rt_03_duplicate_connected_prefix_and_invalid_router_are_rejected() {
    assert!(RouterConfiguration::new(vec![]).is_err());
    let first = Ipv4InterfaceConfig::new(ip(10, 0, 0, 1), 24).unwrap();
    let second = Ipv4InterfaceConfig::new(ip(10, 0, 0, 2), 24).unwrap();
    assert!(RouterConfiguration::new(vec![(interface("eth0"), first)]).is_err());
    assert!(
        RouterConfiguration::new(vec![
            (interface("eth0"), first),
            (interface("eth1"), second)
        ])
        .is_err()
    );
    assert!(
        RouterConfiguration::new(vec![
            (interface("eth0"), first),
            (interface("eth0"), second)
        ])
        .is_err()
    );
}

#[test]
fn rt_04_next_hop_validation_preserves_table() {
    let mut router = router();
    let before = router.clone();
    for (name, next) in [
        ("missing", Some(ip(172, 16, 1, 2))),
        ("eth1", Some(ip(192, 168, 1, 2))),
        ("eth1", Some(ip(172, 16, 1, 1))),
        ("eth1", Some(ip(172, 16, 1, 0))),
        ("eth1", Some(ip(172, 16, 1, 255))),
        ("eth1", Some(ip(224, 0, 0, 1))),
    ] {
        assert!(
            router
                .add_static_route(
                    Ipv4Network::new(ip(10, 0, 0, 0), 8).unwrap(),
                    next,
                    interface(name),
                    0
                )
                .is_err()
        );
        assert_eq!(router, before);
    }
    router
        .add_static_route(
            Ipv4Network::new(ip(10, 0, 0, 0), 8).unwrap(),
            None,
            interface("eth1"),
            0,
        )
        .unwrap();
    assert_eq!(
        router
            .lookup_route(ip(10, 1, 1, 1), |_| true)
            .unwrap()
            .next_hop,
        None
    );
}

#[test]
fn rt_03_reconfiguration_validates_all_routes_before_commit() {
    let mut router = router();
    router
        .add_static_route(
            Ipv4Network::new(ip(10, 20, 0, 0), 16).unwrap(),
            Some(ip(172, 16, 1, 2)),
            interface("eth1"),
            7,
        )
        .unwrap();
    let before = router.clone();
    for (name, address, prefix) in [
        ("eth1", ip(192, 168, 1, 2), 24),
        ("eth0", ip(10, 20, 0, 1), 16),
        ("eth1", ip(172, 17, 1, 1), 24),
        ("missing", ip(172, 17, 1, 1), 24),
    ] {
        assert!(
            router
                .configure_interface(
                    &interface(name),
                    Ipv4InterfaceConfig::new(address, prefix).unwrap()
                )
                .is_err()
        );
        assert_eq!(router, before);
    }
    router
        .configure_interface(
            &interface("eth0"),
            Ipv4InterfaceConfig::new(ip(192, 168, 2, 1), 24).unwrap(),
        )
        .unwrap();
    assert!(router.lookup_route(ip(192, 168, 1, 10), |_| true).is_none());
    assert!(
        router
            .lookup_route(ip(192, 168, 2, 10), |_| true)
            .unwrap()
            .is_connected
    );
    assert_eq!(
        router
            .lookup_route(ip(10, 20, 1, 5), |_| true)
            .unwrap()
            .next_hop,
        Some(ip(172, 16, 1, 2))
    );
}

#[test]
fn rt_05_static_subnet_and_host_route_override_connected_and_default() {
    let mut router = router();
    router
        .add_static_route(
            Ipv4Network::new(ip(0, 0, 0, 0), 0).unwrap(),
            Some(ip(172, 16, 1, 2)),
            interface("eth1"),
            0,
        )
        .unwrap();
    router
        .add_static_route(
            Ipv4Network::new(ip(192, 168, 1, 64), 28).unwrap(),
            Some(ip(172, 16, 1, 2)),
            interface("eth1"),
            999,
        )
        .unwrap();
    router
        .add_static_route(
            Ipv4Network::new(ip(192, 168, 1, 70), 32).unwrap(),
            Some(ip(172, 16, 1, 3)),
            interface("eth1"),
            1000,
        )
        .unwrap();
    for (address, length) in [
        (ip(192, 168, 1, 70), 32),
        (ip(192, 168, 1, 71), 28),
        (ip(192, 168, 1, 10), 24),
        (ip(198, 51, 100, 1), 0),
    ] {
        assert_eq!(
            router
                .lookup_route(address, |_| true)
                .unwrap()
                .network
                .prefix()
                .value(),
            length
        );
    }
}

#[test]
fn rt_04_invalid_next_hop_reason_is_not_a_missing_connected_route() {
    let mut router = router();
    let before = router.clone();
    for next_hop in [
        ip(224, 0, 0, 1),
        ip(127, 0, 0, 1),
        ip(0, 0, 0, 0),
        ip(192, 168, 1, 1),
    ] {
        assert_eq!(
            router.add_static_route(
                Ipv4Network::new(ip(10, 0, 0, 0), 8).unwrap(),
                Some(next_hop),
                interface("eth1"),
                0
            ),
            Err(ConfigurationError::InvalidNextHop)
        );
        assert_eq!(router, before);
    }
}

#[test]
fn rt_validation_error_codes_are_contractual() {
    let mut router = router();
    let network = Ipv4Network::new(ip(10, 0, 0, 0), 8).unwrap();
    assert_eq!(
        router.add_static_route(network, None, interface("absent"), 0),
        Err(ConfigurationError::UnknownInterface)
    );
    assert_eq!(
        router.add_static_route(network, Some(ip(172, 17, 1, 2)), interface("eth1"), 0),
        Err(ConfigurationError::NextHopNotDirectlyConnected)
    );
    router
        .add_static_route(network, None, interface("eth1"), 0)
        .unwrap();
    assert_eq!(
        router.add_static_route(network, None, interface("eth0"), 99),
        Err(ConfigurationError::AmbiguousRouteConfig)
    );
    assert_eq!(
        router.add_static_route(
            Ipv4Network::new(ip(192, 168, 1, 90), 24).unwrap(),
            None,
            interface("eth1"),
            99
        ),
        Err(ConfigurationError::PrefixOverlapsConnectedNetwork)
    );
    assert_eq!(
        router.configure_interface(
            &interface("eth1"),
            Ipv4InterfaceConfig::new(ip(192, 168, 1, 20), 24).unwrap()
        ),
        Err(ConfigurationError::DuplicateConnectedPrefix)
    );
}

#[test]
fn rt_01_connected_routes_are_derived_and_reserved() {
    let router = router();
    assert_eq!(router.routes().len(), 2);
    assert!(
        router
            .routes()
            .iter()
            .all(|r| r.is_connected && r.metric == 0 && r.next_hop.is_none())
    );
    let route = router.lookup_route(ip(192, 168, 1, 44), |_| true).unwrap();
    assert_eq!(route.network.address(), ip(192, 168, 1, 0));
    assert!(
        router
            .lookup_route(ip(192, 168, 1, 44), |_| false)
            .is_none()
    );
    assert_eq!(router.routes().len(), 2);
}

#[test]
fn rt_05_lpm_uses_specificity_not_metric_or_insertion() {
    for order in [[0, 1, 2], [2, 0, 1]] {
        let mut router = router();
        for index in order {
            let (prefix, length, metric) = [
                (ip(10, 0, 0, 0), 8, 1),
                (ip(10, 1, 0, 0), 16, 2),
                (ip(10, 1, 1, 99), 24, 999),
            ][index];
            router
                .add_static_route(
                    Ipv4Network::new(prefix, length).unwrap(),
                    Some(ip(172, 16, 1, 2)),
                    interface("eth1"),
                    metric,
                )
                .unwrap();
        }
        assert_eq!(
            router
                .lookup_route(ip(10, 1, 1, 50), |_| true)
                .unwrap()
                .network
                .prefix()
                .value(),
            24
        );
        assert_eq!(
            router
                .lookup_route(ip(10, 1, 2, 50), |_| true)
                .unwrap()
                .network
                .prefix()
                .value(),
            16
        );
        assert_eq!(
            router
                .lookup_route(ip(10, 2, 2, 50), |_| true)
                .unwrap()
                .network
                .prefix()
                .value(),
            8
        );
        assert!(router.lookup_route(ip(11, 0, 0, 1), |_| true).is_none());
    }
}

#[test]
fn rt_06_operational_predicate_selects_backup_and_recovery() {
    let mut router = router();
    router
        .add_static_route(
            Ipv4Network::new(ip(10, 20, 0, 0), 16).unwrap(),
            Some(ip(172, 16, 1, 2)),
            interface("eth1"),
            5,
        )
        .unwrap();
    router
        .add_static_route(
            Ipv4Network::new(ip(0, 0, 0, 0), 0).unwrap(),
            Some(ip(192, 168, 1, 2)),
            interface("eth0"),
            20,
        )
        .unwrap();
    for (admin, link) in [(false, true), (true, false), (false, false)] {
        let selected = router
            .lookup_route(ip(10, 20, 1, 5), |name| {
                name == &interface("eth0") || (admin && link)
            })
            .unwrap();
        assert_eq!(selected.egress_interface, interface("eth0"));
    }
    assert_eq!(
        router
            .lookup_route(ip(10, 20, 1, 5), |_| true)
            .unwrap()
            .egress_interface,
        interface("eth1")
    );
    assert!(router.lookup_route(ip(10, 20, 1, 5), |_| false).is_none());
}

#[test]
fn cidr_02_normalization_membership_and_idempotence() {
    for (address, prefix, expected) in [
        (ip(192, 168, 1, 10), 24, ip(192, 168, 1, 0)),
        (ip(10, 1, 2, 99), 0, ip(0, 0, 0, 0)),
        (ip(10, 1, 2, 99), 32, ip(10, 1, 2, 99)),
        (ip(172, 16, 0, 1), 31, ip(172, 16, 0, 0)),
    ] {
        let network = Ipv4Network::new(address, prefix).unwrap();
        assert_eq!(network.address(), expected);
        assert!(network.contains(address));
        assert_eq!(
            Ipv4Network::new(network.address(), prefix).unwrap(),
            network
        );
    }
    let subnet = Ipv4Network::new(ip(192, 168, 1, 10), 24).unwrap();
    assert!(!subnet.contains(ip(192, 168, 2, 10)));
    assert!(
        !Ipv4Network::new(ip(10, 1, 2, 99), 32)
            .unwrap()
            .contains(ip(10, 1, 2, 98))
    );
}

#[test]
fn cidr_03_slash31_and_slash32_have_no_directed_broadcast() {
    let pair = Ipv4InterfaceConfig::new(ip(172, 16, 0, 0), 31).unwrap();
    assert!(pair.is_usable_on_link(ip(172, 16, 0, 1)));
    assert!(pair.is_usable_on_link(ip(172, 16, 0, 0)));
    assert_eq!(pair.network().broadcast(), None);
    let host = Ipv4InterfaceConfig::new(ip(10, 0, 0, 7), 32).unwrap();
    assert_eq!(host.network().broadcast(), None);
    assert!(host.is_usable_on_link(host.address()));
}

#[test]
fn cfg_01_gateway_is_usable_on_link_and_not_own_address() {
    let iface = Ipv4InterfaceConfig::new(ip(192, 168, 1, 10), 24).unwrap();
    let config = HostIpv4Config::new(iface, Some(ip(192, 168, 1, 1))).unwrap();
    assert_eq!(config.default_gateway(), Some(ip(192, 168, 1, 1)));
    assert_eq!(
        HostIpv4Config::new(iface, None).unwrap().default_gateway(),
        None
    );
    for gateway in [
        ip(192, 168, 2, 1),
        ip(192, 168, 1, 10),
        ip(192, 168, 1, 0),
        ip(192, 168, 1, 255),
        ip(0, 0, 0, 0),
        ip(127, 0, 0, 1),
        ip(224, 0, 0, 1),
        ip(255, 255, 255, 255),
    ] {
        assert!(
            HostIpv4Config::new(iface, Some(gateway)).is_err(),
            "{gateway:?}"
        );
    }
    assert!(Ipv4InterfaceConfig::new(ip(192, 168, 1, 255), 24).is_err());
}

#[test]
fn cidr_01_prefix_bounds_and_masks() {
    for (length, mask) in [(0, 0), (24, 0xffff_ff00), (31, 0xffff_fffe), (32, u32::MAX)] {
        let prefix = Ipv4PrefixLength::new(length).unwrap();
        assert_eq!(prefix.value(), length);
        assert_eq!(prefix.netmask_u32(), mask);
    }
    assert!(Ipv4PrefixLength::new(33).is_err());
    assert!(Ipv4PrefixLength::new(255).is_err());
}

#[test]
fn cidr_01_all_masks_have_exactly_the_declared_leading_bits() {
    for length in 0..=32 {
        let mask = Ipv4PrefixLength::new(length).unwrap().netmask_u32();
        assert_eq!(mask.count_ones(), u32::from(length));
        assert_eq!(mask.leading_ones(), u32::from(length));
    }
}
