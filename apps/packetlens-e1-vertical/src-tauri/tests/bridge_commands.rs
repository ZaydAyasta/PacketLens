use packetlens_e1_vertical_lib::{contract_version, run_e1_s2_cold};

#[test]
fn bridge_exposes_presentation_v1_and_the_real_e1_s2_cold_snapshot() {
    assert_eq!(contract_version(), "presentation/v1");

    let snapshot = run_e1_s2_cold().expect("E1-S2 cold should export through the bridge");

    assert!(snapshot.contains("\"final_virtual_time_ns\":\"8700\""));
    assert!(snapshot.contains("\"total_emitted_ever\":\"27\""));
    assert!(snapshot.contains("\"trace_seq\":\"27\""));
}
