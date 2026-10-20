use spike_e1_rust::presentation::{
    ContractViolation, PresentationDevice, PresentationInterface, PresentationLink,
    PresentationTopology, SimulationPresentationSnapshot,
};

fn valid_snapshot() -> SimulationPresentationSnapshot {
    SimulationPresentationSnapshot::empty_for_validation(
        "E1-S2",
        "cold",
        PresentationTopology {
            devices: vec![
                PresentationDevice::new("pc1", "host"),
                PresentationDevice::new("sw1", "switch"),
            ],
            interfaces: vec![
                PresentationInterface::new("pc1:eth0", "pc1", "eth0"),
                PresentationInterface::new("sw1:port1", "sw1", "port1"),
            ],
            links: vec![PresentationLink::new("link1", "pc1:eth0", "sw1:port1")],
        },
    )
}

#[test]
fn contract_rejects_a_link_that_references_a_missing_interface() {
    let mut snapshot = valid_snapshot();
    snapshot.topology.links[0].b_interface_id = "missing:eth0".to_owned();

    assert_eq!(
        snapshot.validate(),
        Err(ContractViolation::UnknownLinkInterface {
            link_id: "link1".to_owned(),
            interface_id: "missing:eth0".to_owned(),
        })
    );
}

#[test]
fn contract_rejects_a_link_that_connects_an_interface_to_itself() {
    let mut snapshot = valid_snapshot();
    snapshot.topology.links[0].b_interface_id = "pc1:eth0".to_owned();

    assert_eq!(
        snapshot.validate(),
        Err(ContractViolation::SelfReferentialLink {
            link_id: "link1".to_owned(),
        })
    );
}

#[test]
fn contract_serializes_decimal_quantities_as_json_strings() {
    let snapshot = valid_snapshot();

    assert_eq!(
        snapshot.to_json().unwrap(),
        "{\"schema_version\":\"presentation/v1\",\"scenario_id\":\"E1-S2\",\"run_label\":\"cold\",\"topology\":{\"devices\":[{\"id\":\"pc1\",\"kind\":\"host\"},{\"id\":\"sw1\",\"kind\":\"switch\"}],\"interfaces\":[{\"id\":\"pc1:eth0\",\"device_id\":\"pc1\",\"name\":\"eth0\"},{\"id\":\"sw1:port1\",\"device_id\":\"sw1\",\"name\":\"port1\"}],\"links\":[{\"id\":\"link1\",\"a_interface_id\":\"pc1:eth0\",\"b_interface_id\":\"sw1:port1\"}]},\"execution\":{\"final_virtual_time_ns\":\"0\",\"outcome\":\"Idle\"},\"trace_retention\":{\"total_emitted_ever\":\"0\",\"dropped_by_capacity\":\"0\",\"oldest_available_trace_seq\":null,\"newest_available_trace_seq\":null},\"events\":[]}"
    );
}
