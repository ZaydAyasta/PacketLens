use spike_e1_rust::presentation::{
    PresentationEthernetPayload, PresentationPdu, PresentationTableDelta, export_e1_s2_cold,
};

#[test]
fn e1_s2_cold_exports_the_real_trace_as_a_valid_presentation_snapshot() {
    let snapshot = export_e1_s2_cold().unwrap();

    assert_eq!(snapshot.schema_version, "presentation/v1");
    assert_eq!(snapshot.scenario_id, "E1-S2");
    assert_eq!(snapshot.run_label, "cold");
    assert_eq!(snapshot.execution.outcome, "Quiescent");
    assert_eq!(snapshot.execution.final_virtual_time_ns, "8700");
    assert_eq!(snapshot.events.len(), 27);
    assert_eq!(snapshot.trace_retention.total_emitted_ever, "27");
    assert_eq!(snapshot.trace_retention.dropped_by_capacity, "0");
    assert_eq!(
        snapshot
            .trace_retention
            .oldest_available_trace_seq
            .as_deref(),
        Some("1")
    );
    assert_eq!(
        snapshot
            .trace_retention
            .newest_available_trace_seq
            .as_deref(),
        Some("27")
    );
    assert!(snapshot.validate().is_ok());

    for (index, event) in snapshot.events.iter().enumerate() {
        assert_eq!(event.trace_seq, (index + 1).to_string());
    }

    let arp_transmission = snapshot
        .events
        .iter()
        .find(|event| event.kind == "frame_transmitted" && event.protocols == ["arp"])
        .unwrap();
    // La traza E1 existente emite la primera transmisiÃ³n ARP al tiempo virtual
    // cero; el contrato conserva el valor del motor sin reinterpretarlo.
    assert_eq!(arp_transmission.at_ns, "0");
    match &arp_transmission.pdu.as_ref().unwrap().pdu {
        PresentationPdu::Ethernet {
            src_mac,
            dst_mac,
            payload:
                PresentationEthernetPayload::Arp {
                    operation,
                    sender_ip,
                    target_ip,
                    ..
                },
        } => {
            assert_eq!(src_mac, "00:00:00:00:00:01");
            assert_eq!(dst_mac, "ff:ff:ff:ff:ff:ff");
            assert_eq!(operation, "request");
            assert_eq!(sender_ip, "192.168.1.10");
            assert_eq!(target_ip, "192.168.1.20");
        }
        other => panic!("se esperaba Ethernet/ARP, se obtuvo {other:?}"),
    }

    assert!(snapshot.events.iter().any(|event| matches!(
        &event.table_delta,
        Some(PresentationTableDelta::ArpLearned { ip, mac, expires_at_ns })
            if ip == "192.168.1.20" && mac == "00:00:00:00:00:02" && expires_at_ns == "60000004300"
    )));
    assert!(snapshot.events.iter().any(|event| matches!(
        &event.table_delta,
        Some(PresentationTableDelta::MacLearned { mac, port_interface_id, .. })
            if mac == "00:00:00:00:00:01" && port_interface_id == "sw1:port1"
    )));
    assert!(
        snapshot
            .events
            .iter()
            .any(|event| event.kind == "ping_completed"
                && event.ping.as_ref().and_then(|ping| ping.rtt_ns.as_deref()) == Some("8700"))
    );

    let icmp_transmission = snapshot
        .events
        .iter()
        .find(|event| event.trace_seq == "17")
        .unwrap();
    match &icmp_transmission.pdu.as_ref().unwrap().pdu {
        PresentationPdu::Ethernet {
            payload:
                PresentationEthernetPayload::Ipv4 {
                    src_ip,
                    dst_ip,
                    ttl,
                    ..
                },
            ..
        } => {
            assert_eq!(src_ip, "192.168.1.10");
            assert_eq!(dst_ip, "192.168.1.20");
            assert_eq!(ttl, "64");
        }
        other => panic!("se esperaba Ethernet/IPv4, se obtuvo {other:?}"),
    }
}

#[test]
fn e1_s2_cold_export_is_deterministic() {
    let first = export_e1_s2_cold().unwrap().to_json().unwrap();
    let second = export_e1_s2_cold().unwrap().to_json().unwrap();

    assert_eq!(first, second);
}
