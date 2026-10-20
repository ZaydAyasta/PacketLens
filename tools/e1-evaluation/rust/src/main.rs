use spike_e1_rust::{
    DeviceRef, EndpointRef, Engine, ExecutionReport, InterfaceName, Ipv4Address, LinkRef,
    MacAddress, ScenarioCanonicalReport, VirtualTime,
};
use spike_e1_rust::presentation::export_e1_s2_cold;
use std::time::Instant;

const PC1: MacAddress = MacAddress([0, 0, 0, 0, 0, 1]);
const PC2: MacAddress = MacAddress([0, 0, 0, 0, 0, 2]);
const PC3: MacAddress = MacAddress([0, 0, 0, 0, 0, 3]);
const IP1: Ipv4Address = Ipv4Address([192, 168, 1, 10]);
const IP2: Ipv4Address = Ipv4Address([192, 168, 1, 20]);

fn dev(value: &str) -> DeviceRef { value.into() }
fn iface(value: &str) -> InterfaceName { value.into() }
fn link(value: &str) -> LinkRef { value.into() }
fn ep(device: &str, interface: &str) -> EndpointRef { EndpointRef::new(device, interface) }

fn base_engine() -> Engine {
    let mut engine = Engine::with_default_trace_capacity();
    engine.create_host(dev("pc1"), vec![iface("eth0")]).unwrap();
    engine.create_host(dev("pc2"), vec![iface("eth0")]).unwrap();
    engine.create_host(dev("pc3"), vec![iface("eth0")]).unwrap();
    engine.create_l2_switch(dev("sw1"), vec![iface("port1"), iface("port2"), iface("port3"), iface("port4")]).unwrap();
    engine.configure_interface(ep("pc1", "eth0"), PC1, Some((IP1, 24)), true).unwrap();
    engine.configure_interface(ep("pc2", "eth0"), PC2, Some((IP2, 24)), true).unwrap();
    engine.configure_interface(ep("pc3", "eth0"), PC3, Some((Ipv4Address([192, 168, 1, 30]), 24)), true).unwrap();
    for port in ["port1", "port2", "port3", "port4"] {
        engine.configure_interface(ep("sw1", port), MacAddress::ZERO, None, true).unwrap();
    }
    engine.connect_link(link("link1"), ep("pc1", "eth0"), ep("sw1", "port1")).unwrap();
    engine.connect_link(link("link2"), ep("pc2", "eth0"), ep("sw1", "port2")).unwrap();
    engine.connect_link(link("link3"), ep("pc3", "eth0"), ep("sw1", "port3")).unwrap();
    engine
}

fn loop_engine() -> Engine {
    let mut engine = Engine::new(500).unwrap();
    engine.create_host(dev("pc1"), vec![iface("eth0")]).unwrap();
    engine.create_l2_switch(dev("sw1"), vec![iface("port1"), iface("port2"), iface("port3"), iface("port4")]).unwrap();
    engine.create_l2_switch(dev("sw2"), vec![iface("port1"), iface("port2"), iface("port3"), iface("port4")]).unwrap();
    engine.configure_interface(ep("pc1", "eth0"), PC1, Some((IP1, 24)), true).unwrap();
    for switch in ["sw1", "sw2"] {
        for port in ["port1", "port2", "port3", "port4"] {
            engine.configure_interface(ep(switch, port), MacAddress::ZERO, None, true).unwrap();
        }
    }
    engine.connect_link(link("link1"), ep("pc1", "eth0"), ep("sw1", "port1")).unwrap();
    engine.connect_link(link("link_loop_a"), ep("sw1", "port2"), ep("sw2", "port2")).unwrap();
    engine.connect_link(link("link_loop_b"), ep("sw1", "port3"), ep("sw2", "port3")).unwrap();
    engine
}

fn finish(id: &str, engine: &Engine, report: ExecutionReport) -> String {
    ScenarioCanonicalReport::from_execution(id, &report, engine.get_trace_snapshot()).to_canonical_json()
}

fn scenario_s1() -> String {
    let mut engine = base_engine();
    engine.send_ping(dev("pc1"), IP2, 1, 1).unwrap();
    let report = engine.advance_until(VirtualTime(4_300), 100).unwrap();
    finish("E1-S1", &engine, report)
}

fn scenario_s2() -> String {
    let mut engine = base_engine();
    engine.send_ping(dev("pc1"), IP2, 1, 1).unwrap();
    engine.run_until_quiescent(100).unwrap();
    engine.advance_until(VirtualTime(10_000), 100).unwrap();
    engine.send_ping(dev("pc1"), IP2, 1, 2).unwrap();
    let report = engine.run_until_quiescent(100).unwrap();
    finish("E1-S2", &engine, report)
}

fn scenario_s3a() -> String {
    let mut engine = base_engine();
    engine.set_interface_admin_state(ep("pc2", "eth0"), false).unwrap();
    engine.send_ping(dev("pc1"), IP2, 1, 1).unwrap();
    let report = engine.run_until_quiescent(500).unwrap();
    finish("E1-S3A", &engine, report)
}

fn scenario_s3b() -> String {
    let mut engine = base_engine();
    engine.send_ping(dev("pc1"), IP2, 1, 1).unwrap();
    engine.advance_until(VirtualTime(500), 10).unwrap();
    engine.disconnect_link(&link("link1")).unwrap();
    engine.connect_link(link("link1-new"), ep("pc1", "eth0"), ep("sw1", "port1")).unwrap();
    let report = engine.advance_until(VirtualTime(1_500), 10).unwrap();
    finish("E1-S3B", &engine, report)
}

fn scenario_s4() -> String {
    let mut engine = base_engine();
    engine.send_ping(dev("pc1"), IP2, 1, 1).unwrap();
    engine.run_until_quiescent(100).unwrap();
    engine.advance_until(VirtualTime(20_000), 100).unwrap();
    engine.disconnect_link(&link("link2")).unwrap();
    engine.connect_link(link("link4"), ep("pc2", "eth0"), ep("sw1", "port4")).unwrap();
    engine.send_ping(dev("pc2"), IP1, 2, 1).unwrap();
    let report = engine.run_until_quiescent(100).unwrap();
    finish("E1-S4", &engine, report)
}

fn scenario_s5() -> String {
    let mut engine = base_engine();
    engine.send_ping(dev("pc1"), IP2, 1, 1).unwrap();
    engine.run_until_quiescent(100).unwrap();
    engine.advance_until(VirtualTime(10_000), 100).unwrap();
    engine.send_ping(dev("pc1"), IP2, 1, 2).unwrap();
    engine.run_until_quiescent(100).unwrap();
    engine.advance_until(VirtualTime(60_000_004_299), 100).unwrap();
    engine.advance_until(VirtualTime(60_000_004_300), 100).unwrap();
    engine.advance_until(VirtualTime(61_000_000_000), 100).unwrap();
    engine.send_ping(dev("pc1"), IP2, 1, 3).unwrap();
    let report = engine.run_until_quiescent(100).unwrap();
    finish("E1-S5", &engine, report)
}

fn scenario_s6() -> String {
    let mut engine = loop_engine();
    engine.send_ping(dev("pc1"), Ipv4Address([192, 168, 1, 99]), 9, 1).unwrap();
    let report = engine.step_events(1_000).unwrap();
    finish("E1-S6", &engine, report)
}

fn reports() {
    for report in [scenario_s1(), scenario_s2(), scenario_s3a(), scenario_s3b(), scenario_s4(), scenario_s5(), scenario_s6()] {
        println!("{report}");
    }
}

fn workload(case: &str, iterations: usize) {
    let mut checksum = 0_u64;
    let started = Instant::now();
    for _ in 0..iterations {
        let report = match case {
            "s2-cold" => {
                let mut engine = base_engine();
                engine.send_ping(dev("pc1"), IP2, 1, 1).unwrap();
                engine.run_until_quiescent(100).unwrap()
            }
            "s6" => {
                let mut engine = loop_engine();
                engine.send_ping(dev("pc1"), Ipv4Address([192, 168, 1, 99]), 9, 1).unwrap();
                engine.step_events(1_000).unwrap()
            }
            _ => panic!("unknown workload: {case}"),
        };
        checksum = checksum.wrapping_add(report.current_time.0).wrapping_add(report.processed_events as u64);
    }
    println!("checksum={checksum};elapsed_ns={}", started.elapsed().as_nanos());
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("reports") => reports(),
        Some("presentation-s2-cold") => println!("{}", export_e1_s2_cold().unwrap().to_json().unwrap()),
        Some("workload") => workload(args.get(2).map(String::as_str).expect("workload case"), args.get(3).expect("iterations").parse().expect("integer iterations")),
        _ => panic!("usage: reports | presentation-s2-cold | workload <s2-cold|s6> <iterations>"),
    }
}
