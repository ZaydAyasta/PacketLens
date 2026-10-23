//! DTOs versionados para presentar una ejecuciÃ³n E1.
//!
//! Este mÃ³dulo es una frontera de lectura: sus valores son independientes de
//! `ScenarioCanonicalReport`, no modifican el scheduler y no contienen texto
//! pedagÃ³gico, coordenadas visuales ni causalidad inferida.

use std::collections::BTreeSet;

use crate::{
    ArpOperation, ArpPacket, DeviceRef, DropReason, DroppedPdu, Engine, EngineError, EthernetFrame,
    EthernetPayload, ExecutionReport, IcmpKind, Ipv4Packet, Ipv4Payload, LinkRef, MacAddress,
    ObservableEvent, PingFailureReason, TableUpdateDetail, TraceBufferSnapshot,
};

pub const SCHEMA_VERSION: &str = "presentation/v1";

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PresentationExportError {
    Engine(EngineError),
    Contract(ContractViolation),
    UnexpectedOutcome(String),
}

impl From<EngineError> for PresentationExportError {
    fn from(value: EngineError) -> Self {
        Self::Engine(value)
    }
}

impl From<ContractViolation> for PresentationExportError {
    fn from(value: ContractViolation) -> Self {
        Self::Contract(value)
    }
}

/// Ejecuta el tramo frÃ­o normativo de E1-S2 y lo proyecta sin alterar el
/// scheduler. La topologÃ­a estÃ¡ declarada aquÃ­ como metadato de presentaciÃ³n;
/// el motor sigue siendo la fuente de los eventos y del tiempo virtual.
pub fn export_e1_s2_cold() -> Result<SimulationPresentationSnapshot, PresentationExportError> {
    let mut engine = Engine::with_default_trace_capacity();
    configure_e1_s2_topology(&mut engine)?;
    engine.send_ping(
        DeviceRef::from("pc1"),
        crate::Ipv4Address([192, 168, 1, 20]),
        1,
        1,
    )?;
    let report = engine.run_until_quiescent(100)?;
    let snapshot = from_execution(
        "E1-S2",
        "cold",
        e1_s2_topology(),
        &report,
        engine.get_trace_snapshot(),
    );
    if snapshot.execution.outcome != "Quiescent" {
        return Err(PresentationExportError::UnexpectedOutcome(
            snapshot.execution.outcome,
        ));
    }
    snapshot.validate()?;
    Ok(snapshot)
}

/// Convierte solo informaciÃ³n observable del motor en el contrato de
/// presentaciÃ³n. No recibe ni crea relaciones causales.
pub fn from_execution(
    scenario_id: impl Into<String>,
    run_label: impl Into<String>,
    topology: PresentationTopology,
    report: &ExecutionReport,
    trace: TraceBufferSnapshot,
) -> SimulationPresentationSnapshot {
    SimulationPresentationSnapshot {
        schema_version: SCHEMA_VERSION.to_owned(),
        scenario_id: scenario_id.into(),
        run_label: run_label.into(),
        topology,
        execution: PresentationExecution {
            final_virtual_time_ns: report.current_time.0.to_string(),
            outcome: execution_outcome_code(&report.outcome).to_owned(),
        },
        trace_retention: TraceRetention {
            total_emitted_ever: trace.total_emitted_ever.to_string(),
            dropped_by_capacity: trace.dropped_by_capacity.to_string(),
            oldest_available_trace_seq: trace.oldest_available_seq.map(|value| value.0.to_string()),
            newest_available_trace_seq: trace.newest_available_seq.map(|value| value.0.to_string()),
        },
        events: trace.records.iter().map(project_event).collect(),
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SimulationPresentationSnapshot {
    pub schema_version: String,
    pub scenario_id: String,
    pub run_label: String,
    pub topology: PresentationTopology,
    pub execution: PresentationExecution,
    pub trace_retention: TraceRetention,
    pub events: Vec<PresentationEvent>,
}

impl SimulationPresentationSnapshot {
    /// Constructor limitado a pruebas de validaciÃ³n del contrato. Los
    /// exportadores producen snapshots completos desde una ejecuciÃ³n real.
    pub fn empty_for_validation(
        scenario_id: impl Into<String>,
        run_label: impl Into<String>,
        topology: PresentationTopology,
    ) -> Self {
        Self {
            schema_version: SCHEMA_VERSION.to_owned(),
            scenario_id: scenario_id.into(),
            run_label: run_label.into(),
            topology,
            execution: PresentationExecution {
                final_virtual_time_ns: "0".to_owned(),
                outcome: "Idle".to_owned(),
            },
            trace_retention: TraceRetention {
                total_emitted_ever: "0".to_owned(),
                dropped_by_capacity: "0".to_owned(),
                oldest_available_trace_seq: None,
                newest_available_trace_seq: None,
            },
            events: Vec::new(),
        }
    }

    pub fn validate(&self) -> Result<(), ContractViolation> {
        if self.schema_version != SCHEMA_VERSION {
            return Err(ContractViolation::UnsupportedSchema {
                found: self.schema_version.clone(),
            });
        }
        require_text("scenario_id", &self.scenario_id)?;
        require_text("run_label", &self.run_label)?;

        let mut device_ids = BTreeSet::new();
        for device in &self.topology.devices {
            require_text("topology.devices[].id", &device.id)?;
            require_text("topology.devices[].kind", &device.kind)?;
            if !device_ids.insert(device.id.as_str()) {
                return Err(ContractViolation::DuplicateDeviceId(device.id.clone()));
            }
        }

        let mut interface_ids = BTreeSet::new();
        for interface in &self.topology.interfaces {
            require_text("topology.interfaces[].id", &interface.id)?;
            require_text("topology.interfaces[].device_id", &interface.device_id)?;
            require_text("topology.interfaces[].name", &interface.name)?;
            if !device_ids.contains(interface.device_id.as_str()) {
                return Err(ContractViolation::UnknownInterfaceDevice {
                    interface_id: interface.id.clone(),
                    device_id: interface.device_id.clone(),
                });
            }
            if !interface_ids.insert(interface.id.as_str()) {
                return Err(ContractViolation::DuplicateInterfaceId(
                    interface.id.clone(),
                ));
            }
        }

        let mut link_ids = BTreeSet::new();
        for link in &self.topology.links {
            require_text("topology.links[].id", &link.id)?;
            if !link_ids.insert(link.id.as_str()) {
                return Err(ContractViolation::DuplicateLinkId(link.id.clone()));
            }
            if !interface_ids.contains(link.a_interface_id.as_str()) {
                return Err(ContractViolation::UnknownLinkInterface {
                    link_id: link.id.clone(),
                    interface_id: link.a_interface_id.clone(),
                });
            }
            if !interface_ids.contains(link.b_interface_id.as_str()) {
                return Err(ContractViolation::UnknownLinkInterface {
                    link_id: link.id.clone(),
                    interface_id: link.b_interface_id.clone(),
                });
            }
            if link.a_interface_id == link.b_interface_id {
                return Err(ContractViolation::SelfReferentialLink {
                    link_id: link.id.clone(),
                });
            }
        }

        require_decimal(
            "execution.final_virtual_time_ns",
            &self.execution.final_virtual_time_ns,
        )?;
        require_text("execution.outcome", &self.execution.outcome)?;
        validate_retention(&self.trace_retention, &self.events)?;

        let mut previous_trace_seq: Option<u64> = None;
        let mut previous_at_ns: Option<u64> = None;
        for event in &self.events {
            require_decimal("events[].trace_seq", &event.trace_seq)?;
            require_decimal("events[].at_ns", &event.at_ns)?;
            require_text("events[].kind", &event.kind)?;
            validate_actor(&event.actor, &interface_ids)?;
            validate_optional_endpoint(event.ingress.as_ref(), &interface_ids)?;
            validate_optional_endpoint(event.egress.as_ref(), &interface_ids)?;
            validate_optional_endpoint(event.destination.as_ref(), &interface_ids)?;
            if let Some(link_id) = &event.link_id {
                if !link_ids.contains(link_id.as_str()) {
                    return Err(ContractViolation::UnknownEventLink {
                        trace_seq: event.trace_seq.clone(),
                        link_id: link_id.clone(),
                    });
                }
            }
            if event
                .protocols
                .iter()
                .any(|protocol| protocol.trim().is_empty())
            {
                return Err(ContractViolation::BlankField("events[].protocols[]"));
            }
            if let Some(previous) = previous_trace_seq {
                let current = parse_decimal(&event.trace_seq);
                if current != previous + 1 {
                    return Err(ContractViolation::NonContiguousTraceSequence {
                        previous: (previous).to_string(),
                        current: event.trace_seq.clone(),
                    });
                }
            }
            let at_ns = parse_decimal(&event.at_ns);
            if previous_at_ns.is_some_and(|previous| at_ns < previous) {
                return Err(ContractViolation::NonMonotonicVirtualTime {
                    trace_seq: event.trace_seq.clone(),
                });
            }
            previous_trace_seq = Some(parse_decimal(&event.trace_seq));
            previous_at_ns = Some(at_ns);
        }
        Ok(())
    }

    /// SerializaciÃ³n JSON con orden estable para fixtures y el bridge. A
    /// diferencia de ADR-0002, las cantidades se emiten como cadenas decimales.
    pub fn to_json(&self) -> Result<String, ContractViolation> {
        self.validate()?;
        Ok(format!(
            "{{\"schema_version\":{},\"scenario_id\":{},\"run_label\":{},\"topology\":{},\"execution\":{{\"final_virtual_time_ns\":{},\"outcome\":{}}},\"trace_retention\":{},\"events\":[{}]}}",
            json_string(&self.schema_version),
            json_string(&self.scenario_id),
            json_string(&self.run_label),
            topology_json(&self.topology),
            json_string(&self.execution.final_virtual_time_ns),
            json_string(&self.execution.outcome),
            retention_json(&self.trace_retention),
            self.events
                .iter()
                .map(event_json)
                .collect::<Vec<_>>()
                .join(","),
        ))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PresentationTopology {
    pub devices: Vec<PresentationDevice>,
    pub interfaces: Vec<PresentationInterface>,
    pub links: Vec<PresentationLink>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PresentationDevice {
    pub id: String,
    pub kind: String,
}

impl PresentationDevice {
    pub fn new(id: impl Into<String>, kind: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            kind: kind.into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PresentationInterface {
    pub id: String,
    pub device_id: String,
    pub name: String,
}

impl PresentationInterface {
    pub fn new(
        id: impl Into<String>,
        device_id: impl Into<String>,
        name: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            device_id: device_id.into(),
            name: name.into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PresentationLink {
    pub id: String,
    pub a_interface_id: String,
    pub b_interface_id: String,
}

impl PresentationLink {
    pub fn new(
        id: impl Into<String>,
        a_interface_id: impl Into<String>,
        b_interface_id: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            a_interface_id: a_interface_id.into(),
            b_interface_id: b_interface_id.into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PresentationExecution {
    pub final_virtual_time_ns: String,
    pub outcome: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TraceRetention {
    pub total_emitted_ever: String,
    pub dropped_by_capacity: String,
    pub oldest_available_trace_seq: Option<String>,
    pub newest_available_trace_seq: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PresentationEvent {
    pub trace_seq: String,
    pub at_ns: String,
    pub kind: String,
    pub actor: PresentationActor,
    pub ingress: Option<PresentationEndpoint>,
    pub egress: Option<PresentationEndpoint>,
    pub destination: Option<PresentationEndpoint>,
    pub link_id: Option<String>,
    pub protocols: Vec<String>,
    pub pdu: Option<PresentationPduObservation>,
    pub table_delta: Option<PresentationTableDelta>,
    pub drop_reason: Option<String>,
    pub ping: Option<PresentationPingResult>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PresentationActor {
    pub device_id: String,
    pub interface_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PresentationEndpoint {
    pub device_id: String,
    pub interface_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PresentationPduObservation {
    pub disposition: String,
    pub pdu: PresentationPdu,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PresentationPdu {
    Ethernet {
        src_mac: String,
        dst_mac: String,
        payload: PresentationEthernetPayload,
    },
    Ipv4 {
        src_ip: String,
        dst_ip: String,
        ttl: String,
        payload: PresentationIpv4Payload,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PresentationEthernetPayload {
    Arp {
        operation: String,
        sender_mac: String,
        sender_ip: String,
        target_mac: String,
        target_ip: String,
    },
    Ipv4 {
        src_ip: String,
        dst_ip: String,
        ttl: String,
        payload: PresentationIpv4Payload,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PresentationIpv4Payload {
    Icmp {
        icmp_kind: String,
        identifier: String,
        sequence: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PresentationTableDelta {
    ArpLearned {
        ip: String,
        mac: String,
        expires_at_ns: String,
    },
    ArpEntryExpired {
        ip: String,
    },
    MacLearned {
        mac: String,
        port_interface_id: String,
        expires_at_ns: String,
    },
    MacEntryExpired {
        mac: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PresentationPingResult {
    pub target_ip: String,
    pub identifier: String,
    pub sequence: String,
    pub rtt_ns: Option<String>,
    pub failure_reason: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ContractViolation {
    UnsupportedSchema {
        found: String,
    },
    BlankField(&'static str),
    InvalidDecimal {
        field: &'static str,
        value: String,
    },
    DuplicateDeviceId(String),
    DuplicateInterfaceId(String),
    DuplicateLinkId(String),
    UnknownInterfaceDevice {
        interface_id: String,
        device_id: String,
    },
    UnknownLinkInterface {
        link_id: String,
        interface_id: String,
    },
    SelfReferentialLink {
        link_id: String,
    },
    UnknownEventInterface {
        trace_seq: String,
        interface_id: String,
    },
    UnknownEventLink {
        trace_seq: String,
        link_id: String,
    },
    TraceRetentionMismatch {
        expected_available: String,
        actual_available: String,
    },
    TraceRetentionBoundsMismatch,
    NonContiguousTraceSequence {
        previous: String,
        current: String,
    },
    NonMonotonicVirtualTime {
        trace_seq: String,
    },
}

fn require_text(field: &'static str, value: &str) -> Result<(), ContractViolation> {
    if value.trim().is_empty() {
        Err(ContractViolation::BlankField(field))
    } else {
        Ok(())
    }
}

fn require_decimal(field: &'static str, value: &str) -> Result<(), ContractViolation> {
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(ContractViolation::InvalidDecimal {
            field,
            value: value.to_owned(),
        });
    }
    Ok(())
}

fn parse_decimal(value: &str) -> u64 {
    value
        .parse()
        .expect("validated decimal values fit u64 in E1 v1")
}

fn validate_actor(
    actor: &PresentationActor,
    interfaces: &BTreeSet<&str>,
) -> Result<(), ContractViolation> {
    require_text("events[].actor.device_id", &actor.device_id)?;
    if let Some(interface_id) = &actor.interface_id {
        if !interfaces.contains(interface_id.as_str()) {
            return Err(ContractViolation::UnknownEventInterface {
                trace_seq: "actor".to_owned(),
                interface_id: interface_id.clone(),
            });
        }
    }
    Ok(())
}

fn validate_optional_endpoint(
    endpoint: Option<&PresentationEndpoint>,
    interfaces: &BTreeSet<&str>,
) -> Result<(), ContractViolation> {
    if let Some(endpoint) = endpoint {
        require_text("events[].endpoint.device_id", &endpoint.device_id)?;
        if !interfaces.contains(endpoint.interface_id.as_str()) {
            return Err(ContractViolation::UnknownEventInterface {
                trace_seq: "endpoint".to_owned(),
                interface_id: endpoint.interface_id.clone(),
            });
        }
    }
    Ok(())
}

fn validate_retention(
    retention: &TraceRetention,
    events: &[PresentationEvent],
) -> Result<(), ContractViolation> {
    require_decimal(
        "trace_retention.total_emitted_ever",
        &retention.total_emitted_ever,
    )?;
    require_decimal(
        "trace_retention.dropped_by_capacity",
        &retention.dropped_by_capacity,
    )?;
    let emitted = parse_decimal(&retention.total_emitted_ever);
    let dropped = parse_decimal(&retention.dropped_by_capacity);
    let expected_available =
        emitted
            .checked_sub(dropped)
            .ok_or_else(|| ContractViolation::TraceRetentionMismatch {
                expected_available: "non-negative".to_owned(),
                actual_available: events.len().to_string(),
            })?;
    if expected_available != events.len() as u64 {
        return Err(ContractViolation::TraceRetentionMismatch {
            expected_available: expected_available.to_string(),
            actual_available: events.len().to_string(),
        });
    }
    let bounds_match = match (
        events.first(),
        events.last(),
        &retention.oldest_available_trace_seq,
        &retention.newest_available_trace_seq,
    ) {
        (None, None, None, None) => true,
        (Some(first), Some(last), Some(oldest), Some(newest)) => {
            first.trace_seq == *oldest && last.trace_seq == *newest
        }
        _ => false,
    };
    if !bounds_match {
        return Err(ContractViolation::TraceRetentionBoundsMismatch);
    }
    Ok(())
}

fn configure_e1_s2_topology(engine: &mut Engine) -> Result<(), EngineError> {
    engine.create_host(
        DeviceRef::from("pc1"),
        vec![crate::InterfaceName::from("eth0")],
    )?;
    engine.create_host(
        DeviceRef::from("pc2"),
        vec![crate::InterfaceName::from("eth0")],
    )?;
    engine.create_host(
        DeviceRef::from("pc3"),
        vec![crate::InterfaceName::from("eth0")],
    )?;
    engine.create_l2_switch(
        DeviceRef::from("sw1"),
        vec!["port1", "port2", "port3", "port4"]
            .into_iter()
            .map(crate::InterfaceName::from)
            .collect(),
    )?;
    configure_host(
        engine,
        "pc1",
        MacAddress([0, 0, 0, 0, 0, 1]),
        [192, 168, 1, 10],
    )?;
    configure_host(
        engine,
        "pc2",
        MacAddress([0, 0, 0, 0, 0, 2]),
        [192, 168, 1, 20],
    )?;
    configure_host(
        engine,
        "pc3",
        MacAddress([0, 0, 0, 0, 0, 3]),
        [192, 168, 1, 30],
    )?;
    for port in ["port1", "port2", "port3", "port4"] {
        engine.configure_interface(
            crate::EndpointRef::new("sw1", port),
            MacAddress::ZERO,
            None,
            true,
        )?;
    }
    engine.connect_link(
        LinkRef::from("link1"),
        crate::EndpointRef::new("pc1", "eth0"),
        crate::EndpointRef::new("sw1", "port1"),
    )?;
    engine.connect_link(
        LinkRef::from("link2"),
        crate::EndpointRef::new("pc2", "eth0"),
        crate::EndpointRef::new("sw1", "port2"),
    )?;
    engine.connect_link(
        LinkRef::from("link3"),
        crate::EndpointRef::new("pc3", "eth0"),
        crate::EndpointRef::new("sw1", "port3"),
    )?;
    Ok(())
}

fn configure_host(
    engine: &mut Engine,
    device: &str,
    mac: MacAddress,
    ip: [u8; 4],
) -> Result<(), EngineError> {
    engine.configure_interface(
        crate::EndpointRef::new(device, "eth0"),
        mac,
        Some((crate::Ipv4Address(ip), 24)),
        true,
    )
}

fn e1_s2_topology() -> PresentationTopology {
    PresentationTopology {
        devices: vec![
            PresentationDevice::new("pc1", "host"),
            PresentationDevice::new("pc2", "host"),
            PresentationDevice::new("pc3", "host"),
            PresentationDevice::new("sw1", "l2_switch"),
        ],
        interfaces: vec![
            PresentationInterface::new("pc1:eth0", "pc1", "eth0"),
            PresentationInterface::new("pc2:eth0", "pc2", "eth0"),
            PresentationInterface::new("pc3:eth0", "pc3", "eth0"),
            PresentationInterface::new("sw1:port1", "sw1", "port1"),
            PresentationInterface::new("sw1:port2", "sw1", "port2"),
            PresentationInterface::new("sw1:port3", "sw1", "port3"),
            PresentationInterface::new("sw1:port4", "sw1", "port4"),
        ],
        links: vec![
            PresentationLink::new("link1", "pc1:eth0", "sw1:port1"),
            PresentationLink::new("link2", "pc2:eth0", "sw1:port2"),
            PresentationLink::new("link3", "pc3:eth0", "sw1:port3"),
        ],
    }
}

fn project_event(record: &crate::TraceRecord) -> PresentationEvent {
    let mut event = PresentationEvent {
        trace_seq: record.seq.0.to_string(),
        at_ns: record.at.0.to_string(),
        kind: observable_event_code(&record.event).to_owned(),
        actor: PresentationActor {
            device_id: String::new(),
            interface_id: None,
        },
        ingress: None,
        egress: None,
        destination: None,
        link_id: None,
        protocols: Vec::new(),
        pdu: None,
        table_delta: None,
        drop_reason: None,
        ping: None,
    };
    match &record.event {
        ObservableEvent::FrameTransmitted {
            link,
            from,
            to,
            frame,
            ..
        } => {
            let source = endpoint(from);
            event.actor = actor_from_endpoint(&source);
            event.egress = Some(source);
            event.destination = Some(endpoint(to));
            event.link_id = Some(link.0.clone());
            event.protocols = protocols_for_frame(frame);
            event.pdu = Some(PresentationPduObservation {
                disposition: "outbound".to_owned(),
                pdu: pdu_from_frame(frame),
            });
        }
        ObservableEvent::FrameDelivered {
            link,
            at_endpoint,
            frame,
        } => {
            let destination = endpoint(at_endpoint);
            event.actor = actor_from_endpoint(&destination);
            event.ingress = Some(destination);
            event.link_id = Some(link.0.clone());
            event.protocols = protocols_for_frame(frame);
            event.pdu = Some(PresentationPduObservation {
                disposition: "inbound".to_owned(),
                pdu: pdu_from_frame(frame),
            });
        }
        ObservableEvent::PacketDropped {
            at_device,
            at_interface,
            pdu,
            reason,
        } => {
            event.actor = PresentationActor {
                device_id: at_device.0.clone(),
                interface_id: at_interface
                    .as_ref()
                    .map(|interface| interface_id(at_device, interface)),
            };
            event.protocols = protocols_for_dropped_pdu(pdu);
            event.pdu = Some(PresentationPduObservation {
                disposition: "dropped".to_owned(),
                pdu: pdu_from_dropped(pdu),
            });
            event.drop_reason = Some(drop_reason_code(reason).to_owned());
        }
        ObservableEvent::ArpPacketQueued {
            at_device, packet, ..
        } => {
            event.actor = PresentationActor {
                device_id: at_device.0.clone(),
                interface_id: None,
            };
            event.protocols = vec!["arp".to_owned(), "ipv4".to_owned(), "icmp".to_owned()];
            event.pdu = Some(PresentationPduObservation {
                disposition: "queued".to_owned(),
                pdu: pdu_from_ipv4(packet),
            });
        }
        ObservableEvent::TableUpdated { at_device, update } => {
            event.actor = PresentationActor {
                device_id: at_device.0.clone(),
                interface_id: None,
            };
            event.protocols = protocols_for_table_update(update);
            event.table_delta = Some(table_delta_from(update, at_device));
        }
        ObservableEvent::PingCompleted {
            host,
            target_ip,
            identifier,
            sequence,
            rtt_ns,
        } => {
            event.actor = PresentationActor {
                device_id: host.0.clone(),
                interface_id: None,
            };
            event.protocols = vec!["icmp".to_owned()];
            event.ping = Some(PresentationPingResult {
                target_ip: ip_text(*target_ip),
                identifier: identifier.to_string(),
                sequence: sequence.to_string(),
                rtt_ns: Some(rtt_ns.to_string()),
                failure_reason: None,
            });
        }
        ObservableEvent::PingFailed {
            host,
            target_ip,
            identifier,
            sequence,
            reason,
        } => {
            event.actor = PresentationActor {
                device_id: host.0.clone(),
                interface_id: None,
            };
            event.protocols = vec!["icmp".to_owned()];
            event.ping = Some(PresentationPingResult {
                target_ip: ip_text(*target_ip),
                identifier: identifier.to_string(),
                sequence: sequence.to_string(),
                rtt_ns: None,
                failure_reason: Some(ping_failure_code(reason).to_owned()),
            });
        }
    }
    event
}

fn endpoint(value: &crate::EndpointRef) -> PresentationEndpoint {
    PresentationEndpoint {
        device_id: value.device.0.clone(),
        interface_id: interface_id(&value.device, &value.interface),
    }
}

fn actor_from_endpoint(value: &PresentationEndpoint) -> PresentationActor {
    PresentationActor {
        device_id: value.device_id.clone(),
        interface_id: Some(value.interface_id.clone()),
    }
}

fn interface_id(device: &DeviceRef, interface: &crate::InterfaceName) -> String {
    format!("{}:{}", device.0, interface.0)
}

fn observable_event_code(event: &ObservableEvent) -> &'static str {
    match event {
        ObservableEvent::FrameTransmitted { .. } => "frame_transmitted",
        ObservableEvent::FrameDelivered { .. } => "frame_delivered",
        ObservableEvent::PacketDropped { .. } => "packet_dropped",
        ObservableEvent::ArpPacketQueued { .. } => "arp_packet_queued",
        ObservableEvent::TableUpdated { .. } => "table_updated",
        ObservableEvent::PingCompleted { .. } => "ping_completed",
        ObservableEvent::PingFailed { .. } => "ping_failed",
    }
}

fn execution_outcome_code(outcome: &crate::ExecutionOutcome) -> &'static str {
    match outcome {
        crate::ExecutionOutcome::Idle => "Idle",
        crate::ExecutionOutcome::Quiescent => "Quiescent",
        crate::ExecutionOutcome::TargetTimeReached => "TargetTimeReached",
        crate::ExecutionOutcome::BudgetExhausted => "BudgetExhausted",
        crate::ExecutionOutcome::HaltedBySafetyGuard(_) => "HaltedBySafetyGuard",
    }
}

fn pdu_from_frame(frame: &EthernetFrame) -> PresentationPdu {
    PresentationPdu::Ethernet {
        src_mac: mac_text(frame.src_mac),
        dst_mac: mac_text(frame.dst_mac),
        payload: match &frame.payload {
            EthernetPayload::Arp(packet) => ethernet_arp_payload(packet),
            EthernetPayload::Ipv4(packet) => PresentationEthernetPayload::Ipv4 {
                src_ip: ip_text(packet.src_ip),
                dst_ip: ip_text(packet.dst_ip),
                ttl: packet.ttl.to_string(),
                payload: ipv4_payload(packet),
            },
        },
    }
}

fn pdu_from_ipv4(packet: &Ipv4Packet) -> PresentationPdu {
    PresentationPdu::Ipv4 {
        src_ip: ip_text(packet.src_ip),
        dst_ip: ip_text(packet.dst_ip),
        ttl: packet.ttl.to_string(),
        payload: ipv4_payload(packet),
    }
}

fn pdu_from_dropped(pdu: &DroppedPdu) -> PresentationPdu {
    match pdu {
        DroppedPdu::Ethernet(frame) => pdu_from_frame(frame),
        DroppedPdu::Ipv4(packet) => pdu_from_ipv4(packet),
    }
}

fn ethernet_arp_payload(packet: &ArpPacket) -> PresentationEthernetPayload {
    PresentationEthernetPayload::Arp {
        operation: match packet.operation {
            ArpOperation::Request => "request",
            ArpOperation::Reply => "reply",
        }
        .to_owned(),
        sender_mac: mac_text(packet.sender_mac),
        sender_ip: ip_text(packet.sender_ip),
        target_mac: mac_text(packet.target_mac),
        target_ip: ip_text(packet.target_ip),
    }
}

fn ipv4_payload(packet: &Ipv4Packet) -> PresentationIpv4Payload {
    match &packet.payload {
        Ipv4Payload::IcmpError(_) => unreachable!("E1-only exporter cannot project E2 ICMP errors into presentation/v1"),
        Ipv4Payload::Icmp(icmp) => PresentationIpv4Payload::Icmp {
            icmp_kind: match icmp.kind {
                IcmpKind::EchoRequest => "echo_request",
                IcmpKind::EchoReply => "echo_reply",
                IcmpKind::DestinationHostUnreachable => "destination_host_unreachable",
                IcmpKind::TimeExceeded => "time_exceeded",
            }
            .to_owned(),
            identifier: icmp.identifier.to_string(),
            sequence: icmp.sequence.to_string(),
        },
    }
}

fn protocols_for_frame(frame: &EthernetFrame) -> Vec<String> {
    match &frame.payload {
        EthernetPayload::Arp(_) => vec!["arp".to_owned()],
        EthernetPayload::Ipv4(_) => vec!["ipv4".to_owned(), "icmp".to_owned()],
    }
}

fn protocols_for_dropped_pdu(pdu: &DroppedPdu) -> Vec<String> {
    match pdu {
        DroppedPdu::Ethernet(frame) => protocols_for_frame(frame),
        DroppedPdu::Ipv4(_) => vec!["ipv4".to_owned(), "icmp".to_owned()],
    }
}

fn protocols_for_table_update(update: &TableUpdateDetail) -> Vec<String> {
    match update {
        TableUpdateDetail::ArpLearned { .. } | TableUpdateDetail::ArpEntryExpired { .. } => {
            vec!["arp".to_owned()]
        }
        TableUpdateDetail::MacLearned { .. } | TableUpdateDetail::MacEntryExpired { .. } => {
            vec!["ethernet".to_owned()]
        }
    }
}

fn table_delta_from(update: &TableUpdateDetail, at_device: &DeviceRef) -> PresentationTableDelta {
    match update {
        TableUpdateDetail::ArpLearned {
            ip,
            mac,
            expires_at,
        } => PresentationTableDelta::ArpLearned {
            ip: ip_text(*ip),
            mac: mac_text(*mac),
            expires_at_ns: expires_at.0.to_string(),
        },
        TableUpdateDetail::ArpEntryExpired { ip } => {
            PresentationTableDelta::ArpEntryExpired { ip: ip_text(*ip) }
        }
        TableUpdateDetail::MacLearned {
            mac,
            port,
            expires_at,
        } => PresentationTableDelta::MacLearned {
            mac: mac_text(*mac),
            port_interface_id: interface_id(at_device, port),
            expires_at_ns: expires_at.0.to_string(),
        },
        TableUpdateDetail::MacEntryExpired { mac } => PresentationTableDelta::MacEntryExpired {
            mac: mac_text(*mac),
        },
    }
}

fn drop_reason_code(reason: &DropReason) -> &'static str {
    match reason {
        DropReason::NoRouteToHost | DropReason::TtlExpiredInTransit | DropReason::UnsupportedIpDestination | DropReason::InvalidIpv4Source => unreachable!("E2 drop cannot be projected by the E1-only exporter"),
        DropReason::LinkDisconnectedInFlight => "link_disconnected_in_flight",
        DropReason::IngressInterfaceDown => "ingress_interface_down",
        DropReason::EgressInterfaceDown => "egress_interface_down",
        DropReason::EgressLinkUnavailable => "egress_link_unavailable",
        DropReason::ArpTargetIpMismatch => "arp_target_ip_mismatch",
        DropReason::ArpResolutionFailed => "arp_resolution_failed",
        DropReason::DestinationMacMismatch => "destination_mac_mismatch",
        DropReason::Ipv4DestinationMismatch => "ipv4_destination_mismatch",
    }
}

fn ping_failure_code(reason: &PingFailureReason) -> &'static str {
    match reason {
        PingFailureReason::ArpTimeout => "arp_timeout",
        PingFailureReason::DestinationNetworkUnreachable => {
            unreachable!("E2 failure is not part of the E1 presentation/v1 exporter")
        }
    }
}

fn mac_text(mac: MacAddress) -> String {
    mac.0
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<Vec<_>>()
        .join(":")
}

fn ip_text(ip: crate::Ipv4Address) -> String {
    format!("{}.{}.{}.{}", ip.0[0], ip.0[1], ip.0[2], ip.0[3])
}

fn json_string(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len() + 2);
    escaped.push('"');
    for character in value.chars() {
        match character {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            character if character.is_control() => {
                escaped.push_str(&format!("\\u{:04x}", character as u32))
            }
            character => escaped.push(character),
        }
    }
    escaped.push('"');
    escaped
}

fn optional_json(value: Option<&String>) -> String {
    value.map_or_else(|| "null".to_owned(), |value| json_string(value))
}

fn topology_json(topology: &PresentationTopology) -> String {
    let devices = topology
        .devices
        .iter()
        .map(|device| {
            format!(
                "{{\"id\":{},\"kind\":{}}}",
                json_string(&device.id),
                json_string(&device.kind)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let interfaces = topology
        .interfaces
        .iter()
        .map(|interface| {
            format!(
                "{{\"id\":{},\"device_id\":{},\"name\":{}}}",
                json_string(&interface.id),
                json_string(&interface.device_id),
                json_string(&interface.name)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let links = topology
        .links
        .iter()
        .map(|link| {
            format!(
                "{{\"id\":{},\"a_interface_id\":{},\"b_interface_id\":{}}}",
                json_string(&link.id),
                json_string(&link.a_interface_id),
                json_string(&link.b_interface_id)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!("{{\"devices\":[{devices}],\"interfaces\":[{interfaces}],\"links\":[{links}]}}")
}

fn retention_json(retention: &TraceRetention) -> String {
    format!(
        "{{\"total_emitted_ever\":{},\"dropped_by_capacity\":{},\"oldest_available_trace_seq\":{},\"newest_available_trace_seq\":{}}}",
        json_string(&retention.total_emitted_ever),
        json_string(&retention.dropped_by_capacity),
        optional_json(retention.oldest_available_trace_seq.as_ref()),
        optional_json(retention.newest_available_trace_seq.as_ref()),
    )
}

fn actor_json(actor: &PresentationActor) -> String {
    format!(
        "{{\"device_id\":{},\"interface_id\":{}}}",
        json_string(&actor.device_id),
        optional_json(actor.interface_id.as_ref())
    )
}

fn endpoint_json(endpoint: &PresentationEndpoint) -> String {
    format!(
        "{{\"device_id\":{},\"interface_id\":{}}}",
        json_string(&endpoint.device_id),
        json_string(&endpoint.interface_id)
    )
}

fn optional_endpoint_json(endpoint: Option<&PresentationEndpoint>) -> String {
    endpoint.map_or_else(|| "null".to_owned(), endpoint_json)
}

fn event_json(event: &PresentationEvent) -> String {
    let protocols = event
        .protocols
        .iter()
        .map(|protocol| json_string(protocol))
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"trace_seq\":{},\"at_ns\":{},\"kind\":{},\"actor\":{},\"ingress\":{},\"egress\":{},\"destination\":{},\"link_id\":{},\"protocols\":[{}],\"pdu\":{},\"table_delta\":{},\"drop_reason\":{},\"ping\":{}}}",
        json_string(&event.trace_seq),
        json_string(&event.at_ns),
        json_string(&event.kind),
        actor_json(&event.actor),
        optional_endpoint_json(event.ingress.as_ref()),
        optional_endpoint_json(event.egress.as_ref()),
        optional_endpoint_json(event.destination.as_ref()),
        optional_json(event.link_id.as_ref()),
        protocols,
        event
            .pdu
            .as_ref()
            .map_or_else(|| "null".to_owned(), pdu_observation_json),
        event
            .table_delta
            .as_ref()
            .map_or_else(|| "null".to_owned(), table_delta_json),
        optional_json(event.drop_reason.as_ref()),
        event
            .ping
            .as_ref()
            .map_or_else(|| "null".to_owned(), ping_json),
    )
}

fn pdu_observation_json(observation: &PresentationPduObservation) -> String {
    format!(
        "{{\"disposition\":{},\"pdu\":{}}}",
        json_string(&observation.disposition),
        pdu_json(&observation.pdu)
    )
}

fn pdu_json(pdu: &PresentationPdu) -> String {
    match pdu {
        PresentationPdu::Ethernet {
            src_mac,
            dst_mac,
            payload,
        } => format!(
            "{{\"kind\":\"ethernet\",\"src_mac\":{},\"dst_mac\":{},\"payload\":{}}}",
            json_string(src_mac),
            json_string(dst_mac),
            ethernet_payload_json(payload)
        ),
        PresentationPdu::Ipv4 {
            src_ip,
            dst_ip,
            ttl,
            payload,
        } => format!(
            "{{\"kind\":\"ipv4\",\"src_ip\":{},\"dst_ip\":{},\"ttl\":{},\"payload\":{}}}",
            json_string(src_ip),
            json_string(dst_ip),
            json_string(ttl),
            ipv4_payload_json(payload)
        ),
    }
}

fn ethernet_payload_json(payload: &PresentationEthernetPayload) -> String {
    match payload {
        PresentationEthernetPayload::Arp {
            operation,
            sender_mac,
            sender_ip,
            target_mac,
            target_ip,
        } => format!(
            "{{\"kind\":\"arp\",\"operation\":{},\"sender_mac\":{},\"sender_ip\":{},\"target_mac\":{},\"target_ip\":{}}}",
            json_string(operation),
            json_string(sender_mac),
            json_string(sender_ip),
            json_string(target_mac),
            json_string(target_ip)
        ),
        PresentationEthernetPayload::Ipv4 {
            src_ip,
            dst_ip,
            ttl,
            payload,
        } => format!(
            "{{\"kind\":\"ipv4\",\"src_ip\":{},\"dst_ip\":{},\"ttl\":{},\"payload\":{}}}",
            json_string(src_ip),
            json_string(dst_ip),
            json_string(ttl),
            ipv4_payload_json(payload)
        ),
    }
}

fn ipv4_payload_json(payload: &PresentationIpv4Payload) -> String {
    match payload {
        PresentationIpv4Payload::Icmp {
            icmp_kind,
            identifier,
            sequence,
        } => format!(
            "{{\"kind\":\"icmp\",\"icmp_kind\":{},\"identifier\":{},\"sequence\":{}}}",
            json_string(icmp_kind),
            json_string(identifier),
            json_string(sequence)
        ),
    }
}

fn table_delta_json(delta: &PresentationTableDelta) -> String {
    match delta {
        PresentationTableDelta::ArpLearned {
            ip,
            mac,
            expires_at_ns,
        } => format!(
            "{{\"kind\":\"arp_learned\",\"ip\":{},\"mac\":{},\"expires_at_ns\":{}}}",
            json_string(ip),
            json_string(mac),
            json_string(expires_at_ns)
        ),
        PresentationTableDelta::ArpEntryExpired { ip } => format!(
            "{{\"kind\":\"arp_entry_expired\",\"ip\":{}}}",
            json_string(ip)
        ),
        PresentationTableDelta::MacLearned {
            mac,
            port_interface_id,
            expires_at_ns,
        } => format!(
            "{{\"kind\":\"mac_learned\",\"mac\":{},\"port_interface_id\":{},\"expires_at_ns\":{}}}",
            json_string(mac),
            json_string(port_interface_id),
            json_string(expires_at_ns)
        ),
        PresentationTableDelta::MacEntryExpired { mac } => format!(
            "{{\"kind\":\"mac_entry_expired\",\"mac\":{}}}",
            json_string(mac)
        ),
    }
}

fn ping_json(ping: &PresentationPingResult) -> String {
    format!(
        "{{\"target_ip\":{},\"identifier\":{},\"sequence\":{},\"rtt_ns\":{},\"failure_reason\":{}}}",
        json_string(&ping.target_ip),
        json_string(&ping.identifier),
        json_string(&ping.sequence),
        optional_json(ping.rtt_ns.as_ref()),
        optional_json(ping.failure_reason.as_ref()),
    )
}
