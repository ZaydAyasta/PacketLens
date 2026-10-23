//! Diagnostic projection of configuration values, not a presentation contract.
use spike_e1_rust::Ipv4Address;
use spike_e1_rust::ipv4::{HostIpv4Config, Ipv4InterfaceConfig, Ipv4Network, Ipv4PrefixLength};
use spike_e1_rust::routing::RouterConfiguration;

fn ip(a: u8, b: u8, c: u8, d: u8) -> Ipv4Address {
    Ipv4Address([a, b, c, d])
}
fn address(ip: Ipv4Address) -> String {
    let [a, b, c, d] = ip.0;
    format!("{a}.{b}.{c}.{d}")
}

fn project() -> Vec<String> {
    let mut rows = Vec::new();
    for length in 0..=32 {
        let prefix = Ipv4PrefixLength::new(length).unwrap();
        let network = Ipv4Network::new(ip(192, 168, 1, 10), length).unwrap();
        rows.push(format!(
            "prefix|{length}|{:08x}|{}",
            prefix.netmask_u32(),
            address(network.address())
        ));
    }
    let iface = Ipv4InterfaceConfig::new(ip(192, 168, 1, 10), 24).unwrap();
    for gw in [
        ip(192, 168, 1, 1),
        ip(192, 168, 1, 10),
        ip(192, 168, 2, 1),
        ip(192, 168, 1, 0),
        ip(192, 168, 1, 255),
        ip(0, 0, 0, 0),
        ip(127, 0, 0, 1),
        ip(224, 0, 0, 1),
    ] {
        let outcome = match HostIpv4Config::new(iface, Some(gw)) {
            Ok(_) => "ok".into(),
            Err(error) => format!("{error:?}"),
        };
        rows.push(format!("gateway|{}|{outcome}", address(gw)));
    }
    let mut router = RouterConfiguration::new(vec![
        (
            "eth0".into(),
            Ipv4InterfaceConfig::new(ip(192, 168, 1, 1), 24).unwrap(),
        ),
        (
            "eth1".into(),
            Ipv4InterfaceConfig::new(ip(172, 16, 1, 1), 24).unwrap(),
        ),
    ])
    .unwrap();
    for (network, prefix) in [
        (ip(10, 0, 0, 0), 8),
        (ip(10, 1, 0, 0), 16),
        (ip(10, 1, 1, 99), 24),
        (ip(10, 1, 1, 50), 32),
    ] {
        router
            .add_static_route(
                Ipv4Network::new(network, prefix).unwrap(),
                Some(ip(172, 16, 1, 2)),
                "eth1".into(),
                999,
            )
            .unwrap();
    }
    router
        .add_static_route(
            Ipv4Network::new(ip(0, 0, 0, 0), 0).unwrap(),
            Some(ip(192, 168, 1, 2)),
            "eth0".into(),
            0,
        )
        .unwrap();
    for state in 0..4 {
        for target in [
            ip(10, 1, 1, 50),
            ip(10, 1, 1, 51),
            ip(10, 1, 2, 50),
            ip(10, 2, 2, 50),
            ip(192, 168, 1, 10),
            ip(198, 51, 100, 1),
        ] {
            let route = router.lookup_route(target, |name| {
                (name.0 == "eth0" && state & 1 != 0) || (name.0 == "eth1" && state & 2 != 0)
            });
            let result = route
                .map(|r| {
                    format!(
                        "{}/{}|{}",
                        address(r.network.address()),
                        r.network.prefix().value(),
                        r.egress_interface.0
                    )
                })
                .unwrap_or_else(|| "none".into());
            rows.push(format!("lookup|{state}|{}|{result}", address(target)));
        }
    }
    for hop in [
        ip(224, 0, 0, 1),
        ip(192, 168, 1, 1),
        ip(172, 17, 1, 2),
        ip(172, 16, 1, 255),
    ] {
        let result = router
            .add_static_route(
                Ipv4Network::new(ip(203, 0, 113, 0), 24).unwrap(),
                Some(hop),
                "eth1".into(),
                0,
            )
            .unwrap_err();
        rows.push(format!("next-hop|{}|{result:?}", address(hop)));
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
