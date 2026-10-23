//! Motor discreto, determinista y sin interfaz gráfica para el experimento E1.
//!
//! Las PDUs son semánticas: este crate no modela bytes Ethernet, FCS,
//! serialización, checksums ni tiempo real.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BinaryHeap, VecDeque};

#[cfg(test)]
mod test_allocation {
    use std::alloc::{GlobalAlloc, Layout, System};
    use std::sync::atomic::{AtomicUsize, Ordering};

    pub struct TrackingAllocator;

    static LIVE_BYTES: AtomicUsize = AtomicUsize::new(0);
    static PEAK_LIVE_BYTES: AtomicUsize = AtomicUsize::new(0);

    fn record_allocated(bytes: usize) {
        let live = LIVE_BYTES.fetch_add(bytes, Ordering::Relaxed) + bytes;
        let mut observed_peak = PEAK_LIVE_BYTES.load(Ordering::Relaxed);
        while live > observed_peak {
            match PEAK_LIVE_BYTES.compare_exchange_weak(
                observed_peak,
                live,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(current) => observed_peak = current,
            }
        }
    }

    fn record_deallocated(bytes: usize) {
        LIVE_BYTES.fetch_sub(bytes, Ordering::Relaxed);
    }

    unsafe impl GlobalAlloc for TrackingAllocator {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            let pointer = unsafe { System.alloc(layout) };
            if !pointer.is_null() {
                record_allocated(layout.size());
            }
            pointer
        }

        unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
            let pointer = unsafe { System.alloc_zeroed(layout) };
            if !pointer.is_null() {
                record_allocated(layout.size());
            }
            pointer
        }

        unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
            record_deallocated(layout.size());
            unsafe { System.dealloc(pointer, layout) };
        }

        unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
            let reallocated = unsafe { System.realloc(pointer, layout, new_size) };
            if !reallocated.is_null() {
                if new_size >= layout.size() {
                    record_allocated(new_size - layout.size());
                } else {
                    record_deallocated(layout.size() - new_size);
                }
            }
            reallocated
        }
    }

    pub fn live_bytes() -> usize {
        LIVE_BYTES.load(Ordering::Relaxed)
    }
}

#[cfg(test)]
#[global_allocator]
static TEST_ALLOCATOR: test_allocation::TrackingAllocator = test_allocation::TrackingAllocator;

/// Contrato de lectura para consumidores de presentaciÃ³n. No forma parte del
/// dominio E1 ni del reporte canÃ³nico definido por ADR-0002.
pub mod presentation;
pub mod ipv4;
mod icmp;
#[cfg(test)]
mod icmp_router_tests;
pub use icmp::{IcmpErrorPacket, OriginalDatagramSummary};
pub mod routing;
pub mod router_engine;
mod router_protocol;

pub const LINK_PROPAGATION_DELAY_NS: u64 = 1_000;
pub const DEVICE_PROCESSING_DELAY_NS: u64 = 100;
pub const ARP_REQUEST_TIMEOUT_NS: u64 = 1_000_000_000;
pub const ARP_MAX_ATTEMPTS: u8 = 3;
pub const ARP_CACHE_TTL_NS: u64 = 60_000_000_000;
pub const MAC_TABLE_TTL_NS: u64 = 300_000_000_000;
pub const MAX_ZERO_DELAY_EVENTS: usize = 1_000;
pub const MAX_PENDING_QUEUE_SIZE: usize = 100_000;
pub const DEFAULT_TRACE_CAPACITY: usize = 10_000;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DeviceRef(pub String);

impl From<&str> for DeviceRef {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct InterfaceName(pub String);

impl From<&str> for InterfaceName {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LinkRef(pub String);

impl From<&str> for LinkRef {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EndpointRef {
    pub device: DeviceRef,
    pub interface: InterfaceName,
}

impl EndpointRef {
    pub fn new(device: impl Into<DeviceRef>, interface: impl Into<InterfaceName>) -> Self {
        Self {
            device: device.into(),
            interface: interface.into(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct VirtualTime(pub u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TraceSeq(pub u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MacAddress(pub [u8; 6]);

impl MacAddress {
    pub const ZERO: Self = Self([0; 6]);
    pub const BROADCAST: Self = Self([0xff; 6]);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Ipv4Address(pub [u8; 4]);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArpOperation {
    Request,
    Reply,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArpPacket {
    pub operation: ArpOperation,
    pub sender_mac: MacAddress,
    pub sender_ip: Ipv4Address,
    pub target_mac: MacAddress,
    pub target_ip: Ipv4Address,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IcmpKind {
    EchoRequest,
    EchoReply,
    DestinationHostUnreachable,
    TimeExceeded,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IcmpPacket {
    pub kind: IcmpKind,
    pub identifier: u16,
    pub sequence: u16,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Ipv4Payload {
    Icmp(IcmpPacket),
    IcmpError(IcmpErrorPacket),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ipv4Packet {
    pub src_ip: Ipv4Address,
    pub dst_ip: Ipv4Address,
    pub ttl: u8,
    pub payload: Ipv4Payload,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EthernetPayload {
    Arp(ArpPacket),
    Ipv4(Ipv4Packet),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EthernetFrame {
    pub src_mac: MacAddress,
    pub dst_mac: MacAddress,
    pub payload: EthernetPayload,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DroppedPdu {
    Ethernet(EthernetFrame),
    Ipv4(Ipv4Packet),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DropReason {
    LinkDisconnectedInFlight,
    IngressInterfaceDown,
    EgressInterfaceDown,
    EgressLinkUnavailable,
    ArpTargetIpMismatch,
    ArpResolutionFailed,
    DestinationMacMismatch,
    Ipv4DestinationMismatch,
    NoRouteToHost,
    TtlExpiredInTransit,
    UnsupportedIpDestination,
    InvalidIpv4Source,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PingFailureReason {
    ArpTimeout,
    DestinationNetworkUnreachable,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TableUpdateDetail {
    ArpLearned {
        ip: Ipv4Address,
        mac: MacAddress,
        expires_at: VirtualTime,
    },
    ArpEntryExpired {
        ip: Ipv4Address,
    },
    MacLearned {
        mac: MacAddress,
        port: InterfaceName,
        expires_at: VirtualTime,
    },
    MacEntryExpired {
        mac: MacAddress,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ObservableEvent {
    FrameTransmitted {
        link: LinkRef,
        from: EndpointRef,
        to: EndpointRef,
        arrives_at: VirtualTime,
        frame: EthernetFrame,
    },
    FrameDelivered {
        link: LinkRef,
        at_endpoint: EndpointRef,
        frame: EthernetFrame,
    },
    PacketDropped {
        at_device: DeviceRef,
        at_interface: Option<InterfaceName>,
        pdu: DroppedPdu,
        reason: DropReason,
    },
    ArpPacketQueued {
        at_device: DeviceRef,
        target_ip: Ipv4Address,
        packet: Ipv4Packet,
    },
    TableUpdated {
        at_device: DeviceRef,
        update: TableUpdateDetail,
    },
    PingCompleted {
        host: DeviceRef,
        target_ip: Ipv4Address,
        identifier: u16,
        sequence: u16,
        rtt_ns: u64,
    },
    PingFailed {
        host: DeviceRef,
        target_ip: Ipv4Address,
        identifier: u16,
        sequence: u16,
        reason: PingFailureReason,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TraceRecord {
    pub seq: TraceSeq,
    pub at: VirtualTime,
    pub event: ObservableEvent,
    /// Domain scope for E2 router observations; not part of presentation/v1.
    pub observation_interface: Option<InterfaceName>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TraceBufferSnapshot {
    pub total_emitted_ever: u64,
    pub dropped_by_capacity: u64,
    pub oldest_available_seq: Option<TraceSeq>,
    pub newest_available_seq: Option<TraceSeq>,
    pub records: Vec<TraceRecord>,
}

#[derive(Clone, Debug)]
struct TraceBuffer {
    capacity: usize,
    total_emitted_ever: u64,
    dropped_by_capacity: u64,
    records: VecDeque<TraceRecord>,
}

impl TraceBuffer {
    fn new(capacity: usize) -> Result<Self, EngineError> {
        if capacity == 0 {
            return Err(EngineError::InvalidTraceCapacity);
        }
        Ok(Self {
            capacity,
            total_emitted_ever: 0,
            dropped_by_capacity: 0,
            records: VecDeque::new(),
        })
    }

    fn emit(&mut self, at: VirtualTime, event: ObservableEvent) -> Result<(), EngineError> {
        self.emit_scoped(at, event, None)
    }

    fn emit_scoped(
        &mut self,
        at: VirtualTime,
        event: ObservableEvent,
        scope: Option<InterfaceName>,
    ) -> Result<(), EngineError> {
        self.total_emitted_ever = self
            .total_emitted_ever
            .checked_add(1)
            .ok_or(EngineError::SequenceOverflow)?;
        if self.records.len() == self.capacity {
            self.records.pop_front();
            self.dropped_by_capacity = self
                .dropped_by_capacity
                .checked_add(1)
                .ok_or(EngineError::SequenceOverflow)?;
        }
        self.records.push_back(TraceRecord {
            seq: TraceSeq(self.total_emitted_ever),
            at,
            event,
            observation_interface: scope,
        });
        Ok(())
    }

    fn snapshot(&self) -> TraceBufferSnapshot {
        TraceBufferSnapshot {
            total_emitted_ever: self.total_emitted_ever,
            dropped_by_capacity: self.dropped_by_capacity,
            oldest_available_seq: self.records.front().map(|r| r.seq),
            newest_available_seq: self.records.back().map(|r| r.seq),
            records: self.records.iter().cloned().collect(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SafetyHaltReason {
    ZeroDelayLoop {
        at: VirtualTime,
        events_at_same_time: usize,
    },
    QueueCapacityExceeded {
        pending_count: usize,
        limit: usize,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExecutionOutcome {
    Idle,
    Quiescent,
    TargetTimeReached,
    BudgetExhausted,
    HaltedBySafetyGuard(SafetyHaltReason),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExecutionReport {
    pub outcome: ExecutionOutcome,
    pub processed_events: usize,
    pub current_time: VirtualTime,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EngineError {
    /// Reserved legacy diagnostic; normative router failure paths no longer emit it.
    RouterIcmpErrorPathPending,
    EngineBusy,
    Ipv4Configuration(ipv4::ConfigurationError),
    DuplicateId(String),
    UnknownDevice(DeviceRef),
    UnknownInterface(EndpointRef),
    EndpointAlreadyConnected(EndpointRef),
    LinkEndpointMismatch(LinkRef),
    InvalidHostConfiguration(DeviceRef),
    NoRoute {
        source: DeviceRef,
        target_ip: Ipv4Address,
    },
    DuplicatePing {
        source: DeviceRef,
        target_ip: Ipv4Address,
        identifier: u16,
        sequence: u16,
    },
    TargetTimeInPast {
        current: VirtualTime,
        requested: VirtualTime,
    },
    TimeOverflow,
    SequenceOverflow,
    EpochOverflow,
    InvalidTraceCapacity,
    Cancelled,
    SafetyGuard(SafetyHaltReason),
}

#[derive(Clone, Debug)]
pub enum EngineCommand {
    CreateHost {
        device: DeviceRef,
        interfaces: Vec<InterfaceName>,
    },
    CreateL2Switch {
        device: DeviceRef,
        ports: Vec<InterfaceName>,
    },
    ConfigureInterface {
        endpoint: EndpointRef,
        mac: MacAddress,
        ipv4: Option<(Ipv4Address, u8)>,
        admin_up: bool,
    },
    SetInterfaceAdminState {
        endpoint: EndpointRef,
        admin_up: bool,
    },
    ConnectLink {
        link: LinkRef,
        a: EndpointRef,
        b: EndpointRef,
    },
    DisconnectLink {
        link: LinkRef,
    },
    SendPing {
        source: DeviceRef,
        target_ip: Ipv4Address,
        identifier: u16,
        sequence: u16,
    },
    StepEvents {
        max_steps: usize,
    },
    AdvanceUntil {
        target_time: VirtualTime,
        max_steps_guard: usize,
    },
    RunUntilQuiescent {
        max_steps_guard: usize,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CommandResult {
    Applied,
    Execution(ExecutionReport),
}

#[derive(Clone, Debug)]
struct Interface {
    ipv4_mode: ipv4::Ipv4LinkMode,
    mac: Option<MacAddress>,
    ipv4: Option<(Ipv4Address, u8)>,
    admin_up: bool,
    attached_link: Option<u64>,
}

impl Interface {
    fn unconfigured() -> Self {
        Self {
            ipv4_mode: ipv4::Ipv4LinkMode::MultiAccess,
            mac: None,
            ipv4: None,
            admin_up: false,
            attached_link: None,
        }
    }
}

#[derive(Clone, Debug)]
struct ArpEntry {
    mac: MacAddress,
    epoch: u64,
}

#[derive(Clone, Debug)]
struct MacEntry {
    port: InterfaceName,
    epoch: u64,
}

#[derive(Clone, Debug)]
struct ActiveResolution {
    attempt: u8,
    epoch: u64,
    iface: InterfaceName,
}

#[derive(Clone, Debug)]
struct PendingPacket {
    packet: Ipv4Packet,
    ping: Option<PingKey>,
}

#[derive(Clone, Debug)]
struct Host {
    interfaces: BTreeMap<InterfaceName, Interface>,
    ipv4_configuration: Option<(InterfaceName, ipv4::HostIpv4Config)>,
    arp_cache: BTreeMap<Ipv4Address, ArpEntry>,
    pending_arp_queue: BTreeMap<Ipv4Address, VecDeque<PendingPacket>>,
    resolutions: BTreeMap<Ipv4Address, ActiveResolution>,
}

#[derive(Clone, Debug)]
struct Switch {
    interfaces: BTreeMap<InterfaceName, Interface>,
    mac_table: BTreeMap<MacAddress, MacEntry>,
}

#[derive(Clone, Debug)]
enum Device {
    Host(Host),
    Switch(Switch),
    Router(router_engine::Router),
}

impl Device {
    fn interfaces(&self) -> &BTreeMap<InterfaceName, Interface> {
        match self {
            Self::Host(host) => &host.interfaces,
            Self::Switch(switch) => &switch.interfaces,
            Self::Router(router) => &router.interfaces,
        }
    }
    fn interfaces_mut(&mut self) -> &mut BTreeMap<InterfaceName, Interface> {
        match self {
            Self::Host(host) => &mut host.interfaces,
            Self::Switch(switch) => &mut switch.interfaces,
            Self::Router(router) => &mut router.interfaces,
        }
    }
}

#[derive(Clone, Debug)]
struct Link {
    instance: u64,
    a: EndpointRef,
    b: EndpointRef,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct PingKey {
    source: DeviceRef,
    target_ip: Ipv4Address,
    identifier: u16,
    sequence: u16,
}

#[derive(Clone, Debug)]
enum InternalEvent {
    TransmitFrame {
        from: EndpointRef,
        frame: EthernetFrame,
    },
    FrameArrived {
        link_instance: u64,
        dst: EndpointRef,
        frame: EthernetFrame,
    },
    ArpResolveRetryTimer {
        node: DeviceRef,
        target_ip: Ipv4Address,
        attempt: u8,
        resolution_epoch: u64,
    },
    ArpEntryExpireTimer {
        node: DeviceRef,
        ip: Ipv4Address,
        entry_epoch: u64,
    },
    MacEntryExpireTimer {
        node: DeviceRef,
        mac: MacAddress,
        entry_epoch: u64,
    },
    RouterArpExpireTimer {
        endpoint: EndpointRef,
        ip: Ipv4Address,
        entry_epoch: u64,
    },
    RouterArpRetryTimer {
        endpoint: EndpointRef,
        target_ip: Ipv4Address,
        attempt: u8,
        resolution_epoch: u64,
    },
}

#[derive(Clone, Debug)]
struct ScheduledEvent {
    at: u64,
    sequence: u64,
    event: InternalEvent,
}

impl PartialEq for ScheduledEvent {
    fn eq(&self, other: &Self) -> bool {
        self.at == other.at && self.sequence == other.sequence
    }
}
impl Eq for ScheduledEvent {}
impl PartialOrd for ScheduledEvent {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for ScheduledEvent {
    fn cmp(&self, other: &Self) -> Ordering {
        // BinaryHeap is a max-heap; reverse both keys to obtain the required min-heap.
        other
            .at
            .cmp(&self.at)
            .then_with(|| other.sequence.cmp(&self.sequence))
    }
}

/// Implementación independiente del motor E1. Las claves de enlace son privadas
/// y nunca se reutilizan, por lo que un evento en vuelo no puede confundirse con
/// un cable reconectado en el mismo puerto.
pub struct Engine {
    now: VirtualTime,
    devices: BTreeMap<DeviceRef, Device>,
    links: BTreeMap<LinkRef, Link>,
    links_by_instance: BTreeMap<u64, LinkRef>,
    scheduler: BinaryHeap<ScheduledEvent>,
    // Exact count for the predicate previously evaluated by scanning `scheduler`.
    // Passive ARP/CAM expiry timers are deliberately excluded.
    active_scheduled_events: usize,
    next_event_sequence: u64,
    next_link_instance: u64,
    next_epoch: u64,
    traces: TraceBuffer,
    pings: BTreeMap<PingKey, u64>,
    last_event_at: Option<u64>,
    events_at_same_time: usize,
    cancellation_requested: bool,
}

impl Engine {
    pub fn new(trace_capacity: usize) -> Result<Self, EngineError> {
        Ok(Self {
            now: VirtualTime(0),
            devices: BTreeMap::new(),
            links: BTreeMap::new(),
            links_by_instance: BTreeMap::new(),
            scheduler: BinaryHeap::new(),
            active_scheduled_events: 0,
            next_event_sequence: 0,
            next_link_instance: 0,
            next_epoch: 0,
            traces: TraceBuffer::new(trace_capacity)?,
            pings: BTreeMap::new(),
            last_event_at: None,
            events_at_same_time: 0,
            cancellation_requested: false,
        })
    }

    pub fn with_default_trace_capacity() -> Self {
        Self::new(DEFAULT_TRACE_CAPACITY).expect("the normative default trace capacity is non-zero")
    }

    pub fn now(&self) -> VirtualTime {
        self.now
    }
    pub fn get_trace_snapshot(&self) -> TraceBufferSnapshot {
        self.traces.snapshot()
    }
    pub fn request_cancellation(&mut self) {
        self.cancellation_requested = true;
    }
    pub fn clear_cancellation(&mut self) {
        self.cancellation_requested = false;
    }

    pub fn apply(&mut self, command: EngineCommand) -> Result<CommandResult, EngineError> {
        match command {
            EngineCommand::CreateHost { device, interfaces } => {
                self.create_host(device, interfaces)?;
                Ok(CommandResult::Applied)
            }
            EngineCommand::CreateL2Switch { device, ports } => {
                self.create_l2_switch(device, ports)?;
                Ok(CommandResult::Applied)
            }
            EngineCommand::ConfigureInterface {
                endpoint,
                mac,
                ipv4,
                admin_up,
            } => {
                self.configure_interface(endpoint, mac, ipv4, admin_up)?;
                Ok(CommandResult::Applied)
            }
            EngineCommand::SetInterfaceAdminState { endpoint, admin_up } => {
                self.set_interface_admin_state(endpoint, admin_up)?;
                Ok(CommandResult::Applied)
            }
            EngineCommand::ConnectLink { link, a, b } => {
                self.connect_link(link, a, b)?;
                Ok(CommandResult::Applied)
            }
            EngineCommand::DisconnectLink { link } => {
                self.disconnect_link(&link)?;
                Ok(CommandResult::Applied)
            }
            EngineCommand::SendPing {
                source,
                target_ip,
                identifier,
                sequence,
            } => {
                self.send_ping(source, target_ip, identifier, sequence)?;
                Ok(CommandResult::Applied)
            }
            EngineCommand::StepEvents { max_steps } => {
                Ok(CommandResult::Execution(self.step_events(max_steps)?))
            }
            EngineCommand::AdvanceUntil {
                target_time,
                max_steps_guard,
            } => Ok(CommandResult::Execution(
                self.advance_until(target_time, max_steps_guard)?,
            )),
            EngineCommand::RunUntilQuiescent { max_steps_guard } => Ok(CommandResult::Execution(
                self.run_until_quiescent(max_steps_guard)?,
            )),
        }
    }

    pub fn create_host(
        &mut self,
        device: DeviceRef,
        interfaces: Vec<InterfaceName>,
    ) -> Result<(), EngineError> {
        if self.devices.contains_key(&device) {
            return Err(EngineError::DuplicateId(device.0));
        }
        let ports = unique_interfaces(interfaces)?;
        self.devices.insert(
            device,
            Device::Host(Host {
                ipv4_configuration: None,
                interfaces: ports
                    .into_iter()
                    .map(|p| (p, Interface::unconfigured()))
                    .collect(),
                arp_cache: BTreeMap::new(),
                pending_arp_queue: BTreeMap::new(),
                resolutions: BTreeMap::new(),
            }),
        );
        Ok(())
    }

    pub fn create_l2_switch(
        &mut self,
        device: DeviceRef,
        ports: Vec<InterfaceName>,
    ) -> Result<(), EngineError> {
        if self.devices.contains_key(&device) {
            return Err(EngineError::DuplicateId(device.0));
        }
        let ports = unique_interfaces(ports)?;
        self.devices.insert(
            device,
            Device::Switch(Switch {
                interfaces: ports
                    .into_iter()
                    .map(|p| (p, Interface::unconfigured()))
                    .collect(),
                mac_table: BTreeMap::new(),
            }),
        );
        Ok(())
    }

    pub fn configure_interface(
        &mut self,
        endpoint: EndpointRef,
        mac: MacAddress,
        ipv4: Option<(Ipv4Address, u8)>,
        admin_up: bool,
    ) -> Result<(), EngineError> {
        if matches!(self.devices.get(&endpoint.device), Some(Device::Router(_))) {
            return self.configure_router_port(endpoint, mac, ipv4, admin_up);
        }
        if let Some((_, prefix)) = ipv4
            && prefix > 32
        {
            return Err(EngineError::InvalidHostConfiguration(endpoint.device));
        }
        let updated = if let Some(Device::Host(host)) = self.devices.get(&endpoint.device)
            && let Some((selected, config)) = &host.ipv4_configuration
        {
            self.ensure_ipv4_configuration_idle()?;
            if selected == &endpoint.interface {
                let (address, prefix) = ipv4.ok_or_else(|| {
                    EngineError::InvalidHostConfiguration(endpoint.device.clone())
                })?;
                let value = ipv4::Ipv4InterfaceConfig::new(address, prefix)
                    .map_err(EngineError::Ipv4Configuration)?;
                Some(
                    ipv4::HostIpv4Config::new(value, config.default_gateway())
                        .map_err(EngineError::Ipv4Configuration)?,
                )
            } else {
                if ipv4.is_some() {
                    return Err(EngineError::InvalidHostConfiguration(endpoint.device));
                }
                None
            }
        } else {
            None
        };
        let iface = self.interface_mut(&endpoint)?;
        iface.mac = Some(mac);
        iface.ipv4 = ipv4;
        iface.admin_up = admin_up;
        if let Some(config) = updated
            && let Some(Device::Host(host)) = self.devices.get_mut(&endpoint.device)
        {
            host.ipv4_configuration = Some((endpoint.interface, config));
        }
        Ok(())
    }

    /// Configure E2.1 addressing without activating routed packet processing.
    pub fn configure_host_ipv4(
        &mut self,
        endpoint: EndpointRef,
        mac: MacAddress,
        config: ipv4::HostIpv4Config,
        admin_up: bool,
    ) -> Result<(), EngineError> {
        self.ensure_ipv4_configuration_idle()?;
        self.interface(&endpoint)?;
        let Some(Device::Host(host)) = self.devices.get(&endpoint.device) else {
            return Err(EngineError::InvalidHostConfiguration(endpoint.device));
        };
        if host.interfaces.iter().any(|(name, iface)| {
            name != &endpoint.interface && iface.ipv4.is_some()
        }) {
            return Err(EngineError::InvalidHostConfiguration(endpoint.device));
        }
        let Some(Device::Host(host)) = self.devices.get_mut(&endpoint.device) else {
            return Err(EngineError::InvalidHostConfiguration(endpoint.device));
        };
        let iface = host.interfaces
            .get_mut(&endpoint.interface)
            .ok_or_else(|| EngineError::UnknownInterface(endpoint.clone()))?;
        iface.mac = Some(mac);
        iface.ipv4 = Some((
            config.interface().address(),
            config.interface().network().prefix().value(),
        ));
        iface.admin_up = admin_up;
        host.ipv4_configuration = Some((endpoint.interface, config));
        Ok(())
    }

    pub fn host_ipv4_config(&self, device: &DeviceRef) -> Option<ipv4::HostIpv4Config> {
        match self.devices.get(device) {
            Some(Device::Host(host)) => host.ipv4_configuration.as_ref().map(|(_, config)| *config),
            _ => None,
        }
    }

    fn ensure_ipv4_configuration_idle(&self) -> Result<(), EngineError> {
        if self.has_active_work() || !self.pings.is_empty() {
            Err(EngineError::EngineBusy)
        } else {
            Ok(())
        }
    }

    pub fn set_interface_admin_state(
        &mut self,
        endpoint: EndpointRef,
        admin_up: bool,
    ) -> Result<(), EngineError> {
        self.interface_mut(&endpoint)?.admin_up = admin_up;
        Ok(())
    }

    pub fn connect_link(
        &mut self,
        link: LinkRef,
        a: EndpointRef,
        b: EndpointRef,
    ) -> Result<(), EngineError> {
        if self.links.contains_key(&link) {
            return Err(EngineError::DuplicateId(link.0));
        }
        if a == b {
            return Err(EngineError::LinkEndpointMismatch(link));
        }
        if self.interface(&a)?.attached_link.is_some() {
            return Err(EngineError::EndpointAlreadyConnected(a));
        }
        if self.interface(&b)?.attached_link.is_some() {
            return Err(EngineError::EndpointAlreadyConnected(b));
        }
        self.next_link_instance = self
            .next_link_instance
            .checked_add(1)
            .ok_or(EngineError::SequenceOverflow)?;
        let instance = self.next_link_instance;
        self.interface_mut(&a)?.attached_link = Some(instance);
        self.interface_mut(&b)?.attached_link = Some(instance);
        self.links_by_instance.insert(instance, link.clone());
        self.links.insert(link, Link { instance, a, b });
        Ok(())
    }

    pub fn disconnect_link(&mut self, link: &LinkRef) -> Result<(), EngineError> {
        let removed = self
            .links
            .remove(link)
            .ok_or_else(|| EngineError::DuplicateId(link.0.clone()))?;
        self.links_by_instance.remove(&removed.instance);
        if let Ok(iface) = self.interface_mut(&removed.a)
            && iface.attached_link == Some(removed.instance)
        {
            iface.attached_link = None;
        }
        if let Ok(iface) = self.interface_mut(&removed.b)
            && iface.attached_link == Some(removed.instance)
        {
            iface.attached_link = None;
        }
        Ok(())
    }

    pub fn send_ping(
        &mut self,
        source: DeviceRef,
        target_ip: Ipv4Address,
        identifier: u16,
        sequence: u16,
    ) -> Result<(), EngineError> {
        self.validate_ipv4_topology()?;
        let (endpoint, _, source_ip) = self.host_interface_for(&source, target_ip)?;
        let key = PingKey {
            source: source.clone(),
            target_ip,
            identifier,
            sequence,
        };
        if self.pings.contains_key(&key) {
            return Err(EngineError::DuplicatePing {
                source,
                target_ip,
                identifier,
                sequence,
            });
        }
        self.pings.insert(key.clone(), self.now.0);
        let packet = Ipv4Packet {
            src_ip: source_ip,
            dst_ip: target_ip,
            ttl: 64,
            payload: Ipv4Payload::Icmp(IcmpPacket {
                kind: IcmpKind::EchoRequest,
                identifier,
                sequence,
            }),
        };
        let next_hop = match self.host_next_hop(&source, target_ip) {
            Ok(next) => next,
            Err(EngineError::NoRoute { .. }) => {
                self.pings.remove(&key);
                self.emit(ObservableEvent::PacketDropped {
                    at_device: source.clone(),
                    at_interface: Some(endpoint.interface),
                    pdu: DroppedPdu::Ipv4(packet),
                    reason: DropReason::NoRouteToHost,
                })?;
                return self.emit(ObservableEvent::PingFailed {
                    host: source,
                    target_ip,
                    identifier,
                    sequence,
                    reason: PingFailureReason::DestinationNetworkUnreachable,
                });
            }
            Err(error) => return Err(error),
        };
        self.host_send_ipv4(
            source,
            endpoint.interface,
            next_hop,
            packet,
            Some(key),
            self.now.0,
        )
    }

    pub fn step_events(&mut self, max_steps: usize) -> Result<ExecutionReport, EngineError> {
        let mut processed = 0;
        while processed < max_steps && !self.scheduler.is_empty() {
            match self.process_one() {
                Ok(()) => processed += 1,
                Err(EngineError::SafetyGuard(reason)) => {
                    return Ok(
                        self.report(ExecutionOutcome::HaltedBySafetyGuard(reason), processed)
                    );
                }
                Err(error) => return Err(error),
            }
        }
        let outcome = if self.scheduler.is_empty() {
            ExecutionOutcome::Idle
        } else {
            ExecutionOutcome::BudgetExhausted
        };
        Ok(self.report(outcome, processed))
    }

    pub fn advance_until(
        &mut self,
        target_time: VirtualTime,
        max_steps_guard: usize,
    ) -> Result<ExecutionReport, EngineError> {
        if target_time < self.now {
            return Err(EngineError::TargetTimeInPast {
                current: self.now,
                requested: target_time,
            });
        }
        let mut processed = 0;
        while self
            .scheduler
            .peek()
            .is_some_and(|event| event.at <= target_time.0)
        {
            if processed == max_steps_guard {
                return Ok(self.report(ExecutionOutcome::BudgetExhausted, processed));
            }
            match self.process_one() {
                Ok(()) => processed += 1,
                Err(EngineError::SafetyGuard(reason)) => {
                    return Ok(
                        self.report(ExecutionOutcome::HaltedBySafetyGuard(reason), processed)
                    );
                }
                Err(error) => return Err(error),
            }
        }
        self.now = target_time;
        Ok(self.report(ExecutionOutcome::TargetTimeReached, processed))
    }

    pub fn run_until_quiescent(
        &mut self,
        max_steps_guard: usize,
    ) -> Result<ExecutionReport, EngineError> {
        let mut processed = 0;
        while self.has_active_work() {
            if processed == max_steps_guard {
                return Ok(self.report(ExecutionOutcome::BudgetExhausted, processed));
            }
            match self.process_one() {
                Ok(()) => processed += 1,
                Err(EngineError::SafetyGuard(reason)) => {
                    return Ok(
                        self.report(ExecutionOutcome::HaltedBySafetyGuard(reason), processed)
                    );
                }
                Err(error) => return Err(error),
            }
        }
        Ok(self.report(ExecutionOutcome::Quiescent, processed))
    }

    pub fn arp_entry(&self, host: &DeviceRef, ip: Ipv4Address) -> Option<MacAddress> {
        match self.devices.get(host) {
            Some(Device::Host(h)) => h.arp_cache.get(&ip).map(|entry| entry.mac),
            _ => None,
        }
    }

    pub fn mac_entry(&self, switch: &DeviceRef, mac: MacAddress) -> Option<InterfaceName> {
        match self.devices.get(switch) {
            Some(Device::Switch(s)) => s.mac_table.get(&mac).map(|entry| entry.port.clone()),
            _ => None,
        }
    }

    pub fn mac_table_len(&self, switch: &DeviceRef) -> usize {
        match self.devices.get(switch) {
            Some(Device::Switch(s)) => s.mac_table.len(),
            _ => 0,
        }
    }

    pub fn pending_arp_packets(&self, host: &DeviceRef, target: Ipv4Address) -> usize {
        match self.devices.get(host) {
            Some(Device::Host(h)) => h.pending_arp_queue.get(&target).map_or(0, VecDeque::len),
            _ => 0,
        }
    }

    fn report(&self, outcome: ExecutionOutcome, processed_events: usize) -> ExecutionReport {
        ExecutionReport {
            outcome,
            processed_events,
            current_time: self.now,
        }
    }

    fn interface(&self, endpoint: &EndpointRef) -> Result<&Interface, EngineError> {
        self.devices
            .get(&endpoint.device)
            .ok_or_else(|| EngineError::UnknownDevice(endpoint.device.clone()))?
            .interfaces()
            .get(&endpoint.interface)
            .ok_or_else(|| EngineError::UnknownInterface(endpoint.clone()))
    }

    fn interface_mut(&mut self, endpoint: &EndpointRef) -> Result<&mut Interface, EngineError> {
        self.devices
            .get_mut(&endpoint.device)
            .ok_or_else(|| EngineError::UnknownDevice(endpoint.device.clone()))?
            .interfaces_mut()
            .get_mut(&endpoint.interface)
            .ok_or_else(|| EngineError::UnknownInterface(endpoint.clone()))
    }

    fn host_interface_for(
        &self,
        host: &DeviceRef,
        peer_ip: Ipv4Address,
    ) -> Result<(EndpointRef, MacAddress, Ipv4Address), EngineError> {
        let Some(Device::Host(node)) = self.devices.get(host) else {
            return if self.devices.contains_key(host) {
                Err(EngineError::InvalidHostConfiguration(host.clone()))
            } else {
                Err(EngineError::UnknownDevice(host.clone()))
            };
        };
        if let Some((name, config)) = &node.ipv4_configuration {
            let endpoint = EndpointRef { device: host.clone(), interface: name.clone() };
            let port = self.interface(&endpoint)?;
            if port.admin_up {
                return Ok((endpoint,port.mac.ok_or_else(|| EngineError::InvalidHostConfiguration(host.clone()))?,config.interface().address()));
            }
            return Err(EngineError::NoRoute { source: host.clone(), target_ip: peer_ip });
        }
        for (name, iface) in &node.interfaces {
            if !iface.admin_up {
                continue;
            }
            if let (Some(mac), Some((ip, prefix))) = (iface.mac, iface.ipv4)
                && same_subnet(ip, peer_ip, prefix)
            {
                return Ok((
                    EndpointRef {
                        device: host.clone(),
                        interface: name.clone(),
                    },
                    mac,
                    ip,
                ));
            }
        }
        Err(EngineError::NoRoute {
            source: host.clone(),
            target_ip: peer_ip,
        })
    }

    fn next_epoch_value(&mut self) -> Result<u64, EngineError> {
        self.next_epoch = self
            .next_epoch
            .checked_add(1)
            .ok_or(EngineError::EpochOverflow)?;
        Ok(self.next_epoch)
    }

    fn schedule(&mut self, at: u64, event: InternalEvent) -> Result<(), EngineError> {
        if self.scheduler.len() >= MAX_PENDING_QUEUE_SIZE {
            return Err(EngineError::SafetyGuard(
                SafetyHaltReason::QueueCapacityExceeded {
                    pending_count: self.scheduler.len().saturating_add(1),
                    limit: MAX_PENDING_QUEUE_SIZE,
                },
            ));
        }
        let is_active = self.event_is_active(&event);
        self.next_event_sequence = self
            .next_event_sequence
            .checked_add(1)
            .ok_or(EngineError::SequenceOverflow)?;
        self.scheduler.push(ScheduledEvent {
            at,
            sequence: self.next_event_sequence,
            event,
        });
        if is_active {
            self.active_scheduled_events += 1;
        }
        Ok(())
    }

    fn schedule_after(&mut self, delay: u64, event: InternalEvent) -> Result<(), EngineError> {
        let at = self
            .now
            .0
            .checked_add(delay)
            .ok_or(EngineError::TimeOverflow)?;
        self.schedule(at, event)
    }

    fn emit(&mut self, event: ObservableEvent) -> Result<(), EngineError> {
        self.traces.emit(self.now, event)
    }

    fn has_active_work(&self) -> bool {
        self.active_scheduled_events > 0
    }

    fn event_is_active(&self, event: &InternalEvent) -> bool {
        match event {
            InternalEvent::TransmitFrame { .. } | InternalEvent::FrameArrived { .. } => true,
            InternalEvent::ArpResolveRetryTimer {
                node,
                target_ip,
                attempt,
                resolution_epoch,
            } => {
                matches!(self.devices.get(node), Some(Device::Host(host))
                    if host.resolutions.get(target_ip).is_some_and(|active| active.epoch == *resolution_epoch && active.attempt == *attempt))
            }
            InternalEvent::RouterArpRetryTimer { endpoint,target_ip,attempt,resolution_epoch } =>
                self.router_resolution_active(endpoint,*target_ip,*attempt,*resolution_epoch),
            InternalEvent::ArpEntryExpireTimer { .. }
            | InternalEvent::RouterArpExpireTimer { .. }
            | InternalEvent::MacEntryExpireTimer { .. } => false,
        }
    }

    fn remove_active_scheduled_event(&mut self) {
        self.active_scheduled_events = self
            .active_scheduled_events
            .checked_sub(1)
            .expect("active scheduled event count must match the scheduler");
    }

    fn process_one(&mut self) -> Result<(), EngineError> {
        if self.cancellation_requested {
            return Err(EngineError::Cancelled);
        }
        let next = self
            .scheduler
            .peek()
            .expect("process_one is only called with a non-empty scheduler");
        if self.last_event_at == Some(next.at) && self.events_at_same_time >= MAX_ZERO_DELAY_EVENTS
        {
            return Err(EngineError::SafetyGuard(SafetyHaltReason::ZeroDelayLoop {
                at: VirtualTime(next.at),
                events_at_same_time: self.events_at_same_time.saturating_add(1),
            }));
        }
        let scheduled = self
            .scheduler
            .pop()
            .expect("scheduler was checked as non-empty");
        if scheduled.at < self.now.0 {
            return Err(EngineError::TimeOverflow);
        }
        self.now = VirtualTime(scheduled.at);
        if self.event_is_active(&scheduled.event) {
            self.remove_active_scheduled_event();
        }
        self.handle_event(scheduled.event)?;
        if self.last_event_at == Some(self.now.0) {
            self.events_at_same_time += 1;
        } else {
            self.last_event_at = Some(self.now.0);
            self.events_at_same_time = 1;
        }
        Ok(())
    }

    fn handle_event(&mut self, event: InternalEvent) -> Result<(), EngineError> {
        match event {
            InternalEvent::TransmitFrame { from, frame } => self.transmit_frame(from, frame),
            InternalEvent::FrameArrived {
                link_instance,
                dst,
                frame,
            } => self.frame_arrived(link_instance, dst, frame),
            InternalEvent::ArpResolveRetryTimer {
                node,
                target_ip,
                attempt,
                resolution_epoch,
            } => self.handle_arp_retry(node, target_ip, attempt, resolution_epoch),
            InternalEvent::ArpEntryExpireTimer {
                node,
                ip,
                entry_epoch,
            } => self.expire_arp(node, ip, entry_epoch),
            InternalEvent::MacEntryExpireTimer {
                node,
                mac,
                entry_epoch,
            } => self.expire_mac(node, mac, entry_epoch),
            InternalEvent::RouterArpExpireTimer { endpoint,ip,entry_epoch } => self.expire_router_arp(endpoint,ip,entry_epoch),
            InternalEvent::RouterArpRetryTimer { endpoint,target_ip,attempt,resolution_epoch } => self.retry_router_arp(endpoint,target_ip,attempt,resolution_epoch),
        }
    }

    fn host_send_ipv4(
        &mut self,
        host: DeviceRef,
        iface_name: InterfaceName,
        next_hop: Ipv4Address,
        packet: Ipv4Packet,
        ping: Option<PingKey>,
        transmit_at: u64,
    ) -> Result<(), EngineError> {
        let endpoint = EndpointRef {
            device: host.clone(),
            interface: iface_name.clone(),
        };
        let (source_mac, cache_mac) = {
            let iface = self.interface(&endpoint)?;
            let source_mac = iface
                .mac
                .ok_or_else(|| EngineError::InvalidHostConfiguration(host.clone()))?;
            let cache_mac = match self.devices.get(&host) {
                Some(Device::Host(node)) => node.arp_cache.get(&next_hop).map(|entry| entry.mac),
                _ => None,
            };
            (source_mac, cache_mac)
        };
        if let Some(destination_mac) = cache_mac {
            let frame = EthernetFrame {
                src_mac: source_mac,
                dst_mac: destination_mac,
                payload: EthernetPayload::Ipv4(packet),
            };
            return self.schedule(
                transmit_at,
                InternalEvent::TransmitFrame {
                    from: endpoint,
                    frame,
                },
            );
        }
        self.emit(ObservableEvent::ArpPacketQueued {
            at_device: host.clone(),
            target_ip: next_hop,
            packet: packet.clone(),
        })?;
        let should_start = match self.devices.get(&host) {
            Some(Device::Host(node)) => !node.resolutions.contains_key(&next_hop),
            _ => return Err(EngineError::InvalidHostConfiguration(host)),
        };
        {
            let Some(Device::Host(node)) = self.devices.get_mut(&host) else {
                return Err(EngineError::InvalidHostConfiguration(host));
            };
            node.pending_arp_queue
                .entry(next_hop)
                .or_default()
                .push_back(PendingPacket { packet, ping });
        }
        if !should_start {
            return Ok(());
        }
        let epoch = self.next_epoch_value()?;
        {
            let Some(Device::Host(node)) = self.devices.get_mut(&host) else {
                return Err(EngineError::InvalidHostConfiguration(host));
            };
            node.resolutions.insert(
                next_hop,
                ActiveResolution {
                    attempt: 1,
                    epoch,
                    iface: iface_name.clone(),
                },
            );
        }
        self.schedule_arp_request(host, iface_name, next_hop, transmit_at)?;
        let retry_at = transmit_at
            .checked_add(ARP_REQUEST_TIMEOUT_NS)
            .ok_or(EngineError::TimeOverflow)?;
        self.schedule(
            retry_at,
            InternalEvent::ArpResolveRetryTimer {
                node: endpoint.device,
                target_ip: next_hop,
                attempt: 1,
                resolution_epoch: epoch,
            },
        )
    }

    fn schedule_arp_request(
        &mut self,
        host: DeviceRef,
        iface_name: InterfaceName,
        target_ip: Ipv4Address,
        at: u64,
    ) -> Result<(), EngineError> {
        let endpoint = EndpointRef {
            device: host.clone(),
            interface: iface_name,
        };
        let iface = self.interface(&endpoint)?;
        let mac = iface
            .mac
            .ok_or_else(|| EngineError::InvalidHostConfiguration(host.clone()))?;
        let ip = iface
            .ipv4
            .map(|(ip, _)| ip)
            .ok_or_else(|| EngineError::InvalidHostConfiguration(host.clone()))?;
        let frame = EthernetFrame {
            src_mac: mac,
            dst_mac: MacAddress::BROADCAST,
            payload: EthernetPayload::Arp(ArpPacket {
                operation: ArpOperation::Request,
                sender_mac: mac,
                sender_ip: ip,
                target_mac: MacAddress::ZERO,
                target_ip,
            }),
        };
        self.schedule(
            at,
            InternalEvent::TransmitFrame {
                from: endpoint,
                frame,
            },
        )
    }

    fn transmit_frame(
        &mut self,
        from: EndpointRef,
        frame: EthernetFrame,
    ) -> Result<(), EngineError> {
        let link_instance = match self.interface(&from)?.attached_link {
            Some(id) => id,
            None => {
                return self.emit(ObservableEvent::PacketDropped {
                    at_device: from.device,
                    at_interface: Some(from.interface),
                    pdu: DroppedPdu::Ethernet(frame),
                    reason: DropReason::EgressLinkUnavailable,
                });
            }
        };
        if !self.interface(&from)?.admin_up {
            return self.emit(ObservableEvent::PacketDropped {
                at_device: from.device,
                at_interface: Some(from.interface),
                pdu: DroppedPdu::Ethernet(frame),
                reason: DropReason::EgressInterfaceDown,
            });
        }
        let Some(link_ref) = self.links_by_instance.get(&link_instance).cloned() else {
            return self.emit(ObservableEvent::PacketDropped {
                at_device: from.device,
                at_interface: Some(from.interface),
                pdu: DroppedPdu::Ethernet(frame),
                reason: DropReason::EgressLinkUnavailable,
            });
        };
        let link = self
            .links
            .get(&link_ref)
            .expect("instance index and link map are changed together");
        let destination = if link.a == from {
            link.b.clone()
        } else if link.b == from {
            link.a.clone()
        } else {
            return Err(EngineError::LinkEndpointMismatch(link_ref));
        };
        let arrives_at = self
            .now
            .0
            .checked_add(LINK_PROPAGATION_DELAY_NS)
            .ok_or(EngineError::TimeOverflow)?;
        self.emit(ObservableEvent::FrameTransmitted {
            link: link_ref,
            from,
            to: destination.clone(),
            arrives_at: VirtualTime(arrives_at),
            frame: frame.clone(),
        })?;
        self.schedule(
            arrives_at,
            InternalEvent::FrameArrived {
                link_instance,
                dst: destination,
                frame,
            },
        )
    }

    fn frame_arrived(
        &mut self,
        link_instance: u64,
        dst: EndpointRef,
        frame: EthernetFrame,
    ) -> Result<(), EngineError> {
        let Some(link_ref) = self.links_by_instance.get(&link_instance).cloned() else {
            return self.emit(ObservableEvent::PacketDropped {
                at_device: dst.device,
                at_interface: Some(dst.interface),
                pdu: DroppedPdu::Ethernet(frame),
                reason: DropReason::LinkDisconnectedInFlight,
            });
        };
        let destination_iface = self.interface(&dst)?;
        if destination_iface.attached_link != Some(link_instance) {
            return self.emit(ObservableEvent::PacketDropped {
                at_device: dst.device,
                at_interface: Some(dst.interface),
                pdu: DroppedPdu::Ethernet(frame),
                reason: DropReason::LinkDisconnectedInFlight,
            });
        }
        if !destination_iface.admin_up {
            return self.emit(ObservableEvent::PacketDropped {
                at_device: dst.device,
                at_interface: Some(dst.interface),
                pdu: DroppedPdu::Ethernet(frame),
                reason: DropReason::IngressInterfaceDown,
            });
        }
        self.emit(ObservableEvent::FrameDelivered {
            link: link_ref,
            at_endpoint: dst.clone(),
            frame: frame.clone(),
        })?;
        match self.devices.get(&dst.device) {
            Some(Device::Host(_)) => self.host_received_frame(dst, frame),
            Some(Device::Switch(_)) => self.switch_received_frame(dst, frame),
            Some(Device::Router(_)) => self.router_received_frame(dst, frame),
            None => Err(EngineError::UnknownDevice(dst.device)),
        }
    }

    fn host_received_frame(
        &mut self,
        endpoint: EndpointRef,
        frame: EthernetFrame,
    ) -> Result<(), EngineError> {
        let (own_mac, own_ip) = {
            let iface = self.interface(&endpoint)?;
            (
                iface.mac.ok_or_else(|| {
                    EngineError::InvalidHostConfiguration(endpoint.device.clone())
                })?,
                iface.ipv4.map(|(ip, _)| ip).ok_or_else(|| {
                    EngineError::InvalidHostConfiguration(endpoint.device.clone())
                })?,
            )
        };
        if frame.dst_mac != own_mac && frame.dst_mac != MacAddress::BROADCAST {
            return self.drop_at(endpoint, frame, DropReason::DestinationMacMismatch);
        }
        match frame.payload.clone() {
            EthernetPayload::Arp(arp) => {
                self.host_received_arp(endpoint, own_mac, own_ip, frame, arp)
            }
            EthernetPayload::Ipv4(packet) => {
                self.host_received_ipv4(endpoint, own_ip, frame, packet)
            }
        }
    }

    fn host_received_arp(
        &mut self,
        endpoint: EndpointRef,
        own_mac: MacAddress,
        own_ip: Ipv4Address,
        frame: EthernetFrame,
        arp: ArpPacket,
    ) -> Result<(), EngineError> {
        let valid = match arp.operation {
            ArpOperation::Request => arp.target_ip == own_ip,
            ArpOperation::Reply => arp.target_ip == own_ip && arp.target_mac == own_mac,
        };
        if !valid {
            return self.drop_at(endpoint, frame, DropReason::ArpTargetIpMismatch);
        }
        self.update_arp(&endpoint.device, arp.sender_ip, arp.sender_mac)?;
        match arp.operation {
            ArpOperation::Request => {
                let reply = EthernetFrame {
                    src_mac: own_mac,
                    dst_mac: arp.sender_mac,
                    payload: EthernetPayload::Arp(ArpPacket {
                        operation: ArpOperation::Reply,
                        sender_mac: own_mac,
                        sender_ip: own_ip,
                        target_mac: arp.sender_mac,
                        target_ip: arp.sender_ip,
                    }),
                };
                self.schedule_after(
                    DEVICE_PROCESSING_DELAY_NS,
                    InternalEvent::TransmitFrame {
                        from: endpoint,
                        frame: reply,
                    },
                )
            }
            ArpOperation::Reply => {
                let (pending, resolution_was_active) = {
                    let Some(Device::Host(host)) = self.devices.get_mut(&endpoint.device) else {
                        return Err(EngineError::InvalidHostConfiguration(endpoint.device));
                    };
                    (
                        host.pending_arp_queue
                            .remove(&arp.sender_ip)
                            .unwrap_or_default(),
                        host.resolutions.remove(&arp.sender_ip).is_some(),
                    )
                };
                if resolution_was_active {
                    self.remove_active_scheduled_event();
                }
                let transmit_at = self
                    .now
                    .0
                    .checked_add(DEVICE_PROCESSING_DELAY_NS)
                    .ok_or(EngineError::TimeOverflow)?;
                for pending_packet in pending {
                    self.host_send_ipv4(
                        endpoint.device.clone(),
                        endpoint.interface.clone(),
                        arp.sender_ip,
                        pending_packet.packet,
                        pending_packet.ping,
                        transmit_at,
                    )?;
                }
                Ok(())
            }
        }
    }

    fn host_received_ipv4(
        &mut self,
        endpoint: EndpointRef,
        own_ip: Ipv4Address,
        frame: EthernetFrame,
        packet: Ipv4Packet,
    ) -> Result<(), EngineError> {
        if packet.dst_ip != own_ip {
            return self.drop_at(endpoint, frame, DropReason::Ipv4DestinationMismatch);
        }
        let Ipv4Payload::Icmp(icmp) = packet.payload.clone() else {
            // Error receipt does not close Ping until the lifecycle increment.
            return Ok(());
        };
        match icmp.kind {
            IcmpKind::EchoRequest => {
                let reply = Ipv4Packet {
                    src_ip: own_ip,
                    dst_ip: packet.src_ip,
                    ttl: 64,
                    payload: Ipv4Payload::Icmp(IcmpPacket {
                        kind: IcmpKind::EchoReply,
                        identifier: icmp.identifier,
                        sequence: icmp.sequence,
                    }),
                };
                let at = self
                    .now
                    .0
                    .checked_add(DEVICE_PROCESSING_DELAY_NS)
                    .ok_or(EngineError::TimeOverflow)?;
                let next = match self.host_next_hop(&endpoint.device, packet.src_ip) {
                    Ok(next) => next,
                    Err(EngineError::NoRoute { .. }) => {
                        return self.emit(ObservableEvent::PacketDropped {
                            at_device: endpoint.device,
                            at_interface: Some(endpoint.interface),
                            pdu: DroppedPdu::Ipv4(reply),
                            reason: DropReason::NoRouteToHost,
                        });
                    }
                    Err(error) => return Err(error),
                };
                self.host_send_ipv4(
                    endpoint.device.clone(),
                    endpoint.interface,
                    next,
                    reply,
                    None,
                    at,
                )
            }
            IcmpKind::EchoReply => {
                let key = PingKey {
                    source: endpoint.device.clone(),
                    target_ip: packet.src_ip,
                    identifier: icmp.identifier,
                    sequence: icmp.sequence,
                };
                if let Some(start) = self.pings.remove(&key) {
                    let rtt_ns = self
                        .now
                        .0
                        .checked_sub(start)
                        .ok_or(EngineError::TimeOverflow)?;
                    self.emit(ObservableEvent::PingCompleted {
                        host: endpoint.device,
                        target_ip: packet.src_ip,
                        identifier: icmp.identifier,
                        sequence: icmp.sequence,
                        rtt_ns,
                    })?;
                }
                Ok(())
            }
            IcmpKind::DestinationHostUnreachable | IcmpKind::TimeExceeded => Ok(()),
        }
    }

    fn switch_received_frame(
        &mut self,
        ingress: EndpointRef,
        frame: EthernetFrame,
    ) -> Result<(), EngineError> {
        self.update_mac(&ingress.device, frame.src_mac, ingress.interface.clone())?;
        let egress = {
            let Some(Device::Switch(switch)) = self.devices.get(&ingress.device) else {
                return Err(EngineError::InvalidHostConfiguration(ingress.device));
            };
            let requested = if frame.dst_mac == MacAddress::BROADCAST {
                None
            } else {
                switch
                    .mac_table
                    .get(&frame.dst_mac)
                    .map(|entry| entry.port.clone())
            };
            match requested {
                Some(port) if port != ingress.interface => switch
                    .interfaces
                    .get(&port)
                    .filter(|iface| iface.admin_up && iface.attached_link.is_some())
                    .map(|_| vec![port])
                    .unwrap_or_default(),
                Some(_) => Vec::new(),
                None => switch
                    .interfaces
                    .iter()
                    .filter(|(name, iface)| {
                        **name != ingress.interface
                            && iface.admin_up
                            && iface.attached_link.is_some()
                    })
                    .map(|(name, _)| name.clone())
                    .collect(),
            }
        };
        let at = self
            .now
            .0
            .checked_add(DEVICE_PROCESSING_DELAY_NS)
            .ok_or(EngineError::TimeOverflow)?;
        for port in egress {
            self.schedule(
                at,
                InternalEvent::TransmitFrame {
                    from: EndpointRef {
                        device: ingress.device.clone(),
                        interface: port,
                    },
                    frame: frame.clone(),
                },
            )?;
        }
        Ok(())
    }

    fn update_arp(
        &mut self,
        host: &DeviceRef,
        ip: Ipv4Address,
        mac: MacAddress,
    ) -> Result<(), EngineError> {
        let epoch = self.next_epoch_value()?;
        let expires = self
            .now
            .0
            .checked_add(ARP_CACHE_TTL_NS)
            .ok_or(EngineError::TimeOverflow)?;
        let Some(Device::Host(node)) = self.devices.get_mut(host) else {
            return Err(EngineError::InvalidHostConfiguration(host.clone()));
        };
        node.arp_cache.insert(ip, ArpEntry { mac, epoch });
        self.emit(ObservableEvent::TableUpdated {
            at_device: host.clone(),
            update: TableUpdateDetail::ArpLearned {
                ip,
                mac,
                expires_at: VirtualTime(expires),
            },
        })?;
        self.schedule(
            expires,
            InternalEvent::ArpEntryExpireTimer {
                node: host.clone(),
                ip,
                entry_epoch: epoch,
            },
        )
    }

    fn update_mac(
        &mut self,
        switch: &DeviceRef,
        mac: MacAddress,
        port: InterfaceName,
    ) -> Result<(), EngineError> {
        let epoch = self.next_epoch_value()?;
        let expires = self
            .now
            .0
            .checked_add(MAC_TABLE_TTL_NS)
            .ok_or(EngineError::TimeOverflow)?;
        let Some(Device::Switch(node)) = self.devices.get_mut(switch) else {
            return Err(EngineError::InvalidHostConfiguration(switch.clone()));
        };
        node.mac_table.insert(
            mac,
            MacEntry {
                port: port.clone(),
                epoch,
            },
        );
        self.emit(ObservableEvent::TableUpdated {
            at_device: switch.clone(),
            update: TableUpdateDetail::MacLearned {
                mac,
                port,
                expires_at: VirtualTime(expires),
            },
        })?;
        self.schedule(
            expires,
            InternalEvent::MacEntryExpireTimer {
                node: switch.clone(),
                mac,
                entry_epoch: epoch,
            },
        )
    }

    fn handle_arp_retry(
        &mut self,
        node: DeviceRef,
        target_ip: Ipv4Address,
        attempt: u8,
        resolution_epoch: u64,
    ) -> Result<(), EngineError> {
        let active = match self.devices.get(&node) {
            Some(Device::Host(host)) => host.resolutions.get(&target_ip).cloned(),
            _ => None,
        };
        let Some(active) = active else {
            return Ok(());
        };
        if active.epoch != resolution_epoch || active.attempt != attempt {
            return Ok(());
        }
        if attempt < ARP_MAX_ATTEMPTS {
            let next_attempt = attempt
                .checked_add(1)
                .ok_or(EngineError::SequenceOverflow)?;
            {
                let Some(Device::Host(host)) = self.devices.get_mut(&node) else {
                    return Err(EngineError::InvalidHostConfiguration(node));
                };
                if let Some(current) = host.resolutions.get_mut(&target_ip) {
                    current.attempt = next_attempt;
                }
            }
            let iface_is_ready = self
                .interface(&EndpointRef {
                    device: node.clone(),
                    interface: active.iface.clone(),
                })?
                .admin_up
                && self
                    .interface(&EndpointRef {
                        device: node.clone(),
                        interface: active.iface.clone(),
                    })?
                    .attached_link
                    .is_some();
            if iface_is_ready {
                self.schedule_arp_request(
                    node.clone(),
                    active.iface.clone(),
                    target_ip,
                    self.now.0,
                )?;
            }
            let retry_at = self
                .now
                .0
                .checked_add(ARP_REQUEST_TIMEOUT_NS)
                .ok_or(EngineError::TimeOverflow)?;
            self.schedule(
                retry_at,
                InternalEvent::ArpResolveRetryTimer {
                    node,
                    target_ip,
                    attempt: next_attempt,
                    resolution_epoch,
                },
            )
        } else {
            let pending = {
                let Some(Device::Host(host)) = self.devices.get_mut(&node) else {
                    return Err(EngineError::InvalidHostConfiguration(node));
                };
                host.resolutions.remove(&target_ip);
                host.pending_arp_queue
                    .remove(&target_ip)
                    .unwrap_or_default()
            };
            for pending_packet in pending {
                self.emit(ObservableEvent::PacketDropped {
                    at_device: node.clone(),
                    at_interface: None,
                    pdu: DroppedPdu::Ipv4(pending_packet.packet),
                    reason: DropReason::ArpResolutionFailed,
                })?;
                if let Some(key) = pending_packet.ping {
                    self.pings.remove(&key);
                    self.emit(ObservableEvent::PingFailed {
                        host: key.source,
                        target_ip: key.target_ip,
                        identifier: key.identifier,
                        sequence: key.sequence,
                        reason: PingFailureReason::ArpTimeout,
                    })?;
                }
            }
            Ok(())
        }
    }

    fn expire_arp(
        &mut self,
        node: DeviceRef,
        ip: Ipv4Address,
        entry_epoch: u64,
    ) -> Result<(), EngineError> {
        let should_expire = matches!(self.devices.get(&node), Some(Device::Host(host)) if host.arp_cache.get(&ip).is_some_and(|entry| entry.epoch == entry_epoch));
        if should_expire {
            if let Some(Device::Host(host)) = self.devices.get_mut(&node) {
                host.arp_cache.remove(&ip);
            }
            self.emit(ObservableEvent::TableUpdated {
                at_device: node,
                update: TableUpdateDetail::ArpEntryExpired { ip },
            })?;
        }
        Ok(())
    }

    fn expire_mac(
        &mut self,
        node: DeviceRef,
        mac: MacAddress,
        entry_epoch: u64,
    ) -> Result<(), EngineError> {
        let should_expire = matches!(self.devices.get(&node), Some(Device::Switch(switch)) if switch.mac_table.get(&mac).is_some_and(|entry| entry.epoch == entry_epoch));
        if should_expire {
            if let Some(Device::Switch(switch)) = self.devices.get_mut(&node) {
                switch.mac_table.remove(&mac);
            }
            self.emit(ObservableEvent::TableUpdated {
                at_device: node,
                update: TableUpdateDetail::MacEntryExpired { mac },
            })?;
        }
        Ok(())
    }

    fn drop_at(
        &mut self,
        endpoint: EndpointRef,
        frame: EthernetFrame,
        reason: DropReason,
    ) -> Result<(), EngineError> {
        self.emit(ObservableEvent::PacketDropped {
            at_device: endpoint.device,
            at_interface: Some(endpoint.interface),
            pdu: DroppedPdu::Ethernet(frame),
            reason,
        })
    }
}

fn unique_interfaces(ports: Vec<InterfaceName>) -> Result<Vec<InterfaceName>, EngineError> {
    let mut ordered = BTreeMap::new();
    for port in ports {
        if ordered.insert(port.clone(), ()).is_some() {
            return Err(EngineError::DuplicateId(port.0));
        }
    }
    Ok(ordered.into_keys().collect())
}

fn same_subnet(a: Ipv4Address, b: Ipv4Address, prefix: u8) -> bool {
    let a = u32::from_be_bytes(a.0);
    let b = u32::from_be_bytes(b.0);
    let mask = if prefix == 0 {
        0
    } else {
        u32::MAX << (32 - prefix)
    };
    (a & mask) == (b & mask)
}

/// Reporte canónico para la comparación posterior con C#. Los contadores de
/// eventos internos son diagnósticos, conforme al addendum A3.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScenarioCanonicalReport {
    pub scenario_id: String,
    pub final_virtual_time_ns: u64,
    pub execution_outcome: ExecutionOutcome,
    pub total_internal_events_processed: usize,
    pub trace_summary: TraceBufferSnapshot,
}

impl ScenarioCanonicalReport {
    pub fn from_execution(
        scenario_id: impl Into<String>,
        report: &ExecutionReport,
        trace_summary: TraceBufferSnapshot,
    ) -> Self {
        Self {
            scenario_id: scenario_id.into(),
            final_virtual_time_ns: report.current_time.0,
            execution_outcome: report.outcome.clone(),
            total_internal_events_processed: report.processed_events,
            trace_summary,
        }
    }

    /// JSON sin dependencias externas, con orden fijo de claves. Los detalles
    /// de PDU usan una gramática deliberadamente estable entre lenguajes.
    pub fn to_canonical_json(&self) -> String {
        let mut json = format!(
            "{{\"scenario_id\":\"{}\",\"final_virtual_time_ns\":{},\"execution_outcome\":\"{}\",\"diagnostics\":{{\"total_internal_events_processed\":{}}},\"trace_summary\":{{\"total_emitted_ever\":{},\"dropped_by_capacity\":{},\"oldest_available_seq\":{},\"newest_available_seq\":{},\"records\":[",
            json_escape(&self.scenario_id),
            self.final_virtual_time_ns,
            outcome_name(&self.execution_outcome),
            self.total_internal_events_processed,
            self.trace_summary.total_emitted_ever,
            self.trace_summary.dropped_by_capacity,
            optional_seq(self.trace_summary.oldest_available_seq),
            optional_seq(self.trace_summary.newest_available_seq),
        );
        for (index, record) in self.trace_summary.records.iter().enumerate() {
            if index > 0 {
                json.push(',');
            }
            json.push_str(&format!(
                "{{\"seq\":{},\"at_ns\":{},\"kind\":\"{}\",\"details\":\"{}\"}}",
                record.seq.0,
                record.at.0,
                observable_kind(&record.event),
                json_escape(&observable_details(&record.event))
            ));
        }
        json.push_str("]}}");
        json
    }
}

fn optional_seq(value: Option<TraceSeq>) -> String {
    value
        .map(|seq| seq.0.to_string())
        .unwrap_or_else(|| "null".to_owned())
}
fn json_escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}
fn outcome_name(outcome: &ExecutionOutcome) -> &'static str {
    match outcome {
        ExecutionOutcome::Idle => "Idle",
        ExecutionOutcome::Quiescent => "Quiescent",
        ExecutionOutcome::TargetTimeReached => "TargetTimeReached",
        ExecutionOutcome::BudgetExhausted => "BudgetExhausted",
        ExecutionOutcome::HaltedBySafetyGuard(_) => "HaltedBySafetyGuard",
    }
}
fn observable_kind(event: &ObservableEvent) -> &'static str {
    match event {
        ObservableEvent::FrameTransmitted { .. } => "FrameTransmitted",
        ObservableEvent::FrameDelivered { .. } => "FrameDelivered",
        ObservableEvent::PacketDropped { .. } => "PacketDropped",
        ObservableEvent::ArpPacketQueued { .. } => "ArpPacketQueued",
        ObservableEvent::TableUpdated { .. } => "TableUpdated",
        ObservableEvent::PingCompleted { .. } => "PingCompleted",
        ObservableEvent::PingFailed { .. } => "PingFailed",
    }
}
fn observable_details(event: &ObservableEvent) -> String {
    match event {
        ObservableEvent::FrameTransmitted {
            link,
            from,
            to,
            arrives_at,
            frame,
        } => format!(
            "link={};from={}:{};to={}:{};arrives={};frame={}",
            link.0,
            from.device.0,
            from.interface.0,
            to.device.0,
            to.interface.0,
            arrives_at.0,
            frame_text(frame)
        ),
        ObservableEvent::FrameDelivered {
            link,
            at_endpoint,
            frame,
        } => format!(
            "link={};at={}:{};frame={}",
            link.0,
            at_endpoint.device.0,
            at_endpoint.interface.0,
            frame_text(frame)
        ),
        ObservableEvent::PacketDropped {
            at_device,
            at_interface,
            pdu,
            reason,
        } => format!(
            "device={};interface={};reason={:?};pdu={}",
            at_device.0,
            at_interface.as_ref().map(|x| x.0.as_str()).unwrap_or("-"),
            reason,
            dropped_pdu_text(pdu)
        ),
        ObservableEvent::ArpPacketQueued {
            at_device,
            target_ip,
            packet,
        } => format!(
            "device={};target={};packet={}",
            at_device.0,
            ip_text(*target_ip),
            ipv4_text(packet)
        ),
        ObservableEvent::TableUpdated { at_device, update } => {
            format!("device={};update={:?}", at_device.0, update)
        }
        ObservableEvent::PingCompleted {
            host,
            target_ip,
            identifier,
            sequence,
            rtt_ns,
        } => format!(
            "host={};target={};id={};seq={};rtt={}",
            host.0,
            ip_text(*target_ip),
            identifier,
            sequence,
            rtt_ns
        ),
        ObservableEvent::PingFailed {
            host,
            target_ip,
            identifier,
            sequence,
            reason,
        } => format!(
            "host={};target={};id={};seq={};reason={:?}",
            host.0,
            ip_text(*target_ip),
            identifier,
            sequence,
            reason
        ),
    }
}
fn mac_text(mac: MacAddress) -> String {
    mac.0
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<Vec<_>>()
        .join(":")
}
fn ip_text(ip: Ipv4Address) -> String {
    format!("{}.{}.{}.{}", ip.0[0], ip.0[1], ip.0[2], ip.0[3])
}
fn frame_text(frame: &EthernetFrame) -> String {
    format!(
        "eth(src={},dst={},payload={})",
        mac_text(frame.src_mac),
        mac_text(frame.dst_mac),
        match &frame.payload {
            EthernetPayload::Arp(arp) => format!(
                "arp({:?},sender_mac={},sender_ip={},target_mac={},target_ip={})",
                arp.operation,
                mac_text(arp.sender_mac),
                ip_text(arp.sender_ip),
                mac_text(arp.target_mac),
                ip_text(arp.target_ip)
            ),
            EthernetPayload::Ipv4(packet) => ipv4_text(packet),
        }
    )
}
fn ipv4_text(packet: &Ipv4Packet) -> String {
    format!(
        "ipv4(src={},dst={},ttl={},payload={:?})",
        ip_text(packet.src_ip),
        ip_text(packet.dst_ip),
        packet.ttl,
        packet.payload
    )
}
fn dropped_pdu_text(pdu: &DroppedPdu) -> String {
    match pdu {
        DroppedPdu::Ethernet(frame) => frame_text(frame),
        DroppedPdu::Ipv4(packet) => ipv4_text(packet),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::hint::black_box;
    use std::time::{Duration, Instant};

    const PC1: MacAddress = MacAddress([0, 0, 0, 0, 0, 1]);
    const PC2: MacAddress = MacAddress([0, 0, 0, 0, 0, 2]);
    const PC3: MacAddress = MacAddress([0, 0, 0, 0, 0, 3]);
    const IP1: Ipv4Address = Ipv4Address([192, 168, 1, 10]);
    const IP2: Ipv4Address = Ipv4Address([192, 168, 1, 20]);
    const IP3: Ipv4Address = Ipv4Address([192, 168, 1, 30]);

    fn ep(device: &str, interface: &str) -> EndpointRef {
        EndpointRef::new(device, interface)
    }
    fn dev(value: &str) -> DeviceRef {
        DeviceRef::from(value)
    }
    fn iface(value: &str) -> InterfaceName {
        InterfaceName::from(value)
    }
    fn link(value: &str) -> LinkRef {
        LinkRef::from(value)
    }

    fn base_engine() -> Engine {
        let mut engine = Engine::with_default_trace_capacity();
        engine.create_host(dev("pc1"), vec![iface("eth0")]).unwrap();
        engine.create_host(dev("pc2"), vec![iface("eth0")]).unwrap();
        engine.create_host(dev("pc3"), vec![iface("eth0")]).unwrap();
        engine
            .create_l2_switch(
                dev("sw1"),
                vec![
                    iface("port1"),
                    iface("port2"),
                    iface("port3"),
                    iface("port4"),
                ],
            )
            .unwrap();
        engine
            .configure_interface(ep("pc1", "eth0"), PC1, Some((IP1, 24)), true)
            .unwrap();
        engine
            .configure_interface(ep("pc2", "eth0"), PC2, Some((IP2, 24)), true)
            .unwrap();
        engine
            .configure_interface(ep("pc3", "eth0"), PC3, Some((IP3, 24)), true)
            .unwrap();
        for port in ["port1", "port2", "port3", "port4"] {
            engine
                .configure_interface(ep("sw1", port), MacAddress::ZERO, None, true)
                .unwrap();
        }
        engine
            .connect_link(link("link1"), ep("pc1", "eth0"), ep("sw1", "port1"))
            .unwrap();
        engine
            .connect_link(link("link2"), ep("pc2", "eth0"), ep("sw1", "port2"))
            .unwrap();
        engine
            .connect_link(link("link3"), ep("pc3", "eth0"), ep("sw1", "port3"))
            .unwrap();
        engine
    }

    #[derive(Clone, Copy)]
    struct SchedulerBenchmarkCase {
        name: &'static str,
        active_frames: usize,
        passive_timers: usize,
    }

    fn benchmark_frame() -> EthernetFrame {
        EthernetFrame {
            src_mac: PC1,
            dst_mac: MacAddress::BROADCAST,
            payload: EthernetPayload::Arp(ArpPacket {
                operation: ArpOperation::Request,
                sender_mac: PC1,
                sender_ip: IP1,
                target_mac: MacAddress::ZERO,
                target_ip: IP2,
            }),
        }
    }

    fn engine_with_benchmark_workload(case: SchedulerBenchmarkCase) -> (Engine, usize) {
        let mut engine = base_engine();
        let live_before_scheduling = test_allocation::live_bytes();

        for index in 0..case.passive_timers {
            engine
                .schedule(
                    (index as u64) + 1,
                    InternalEvent::MacEntryExpireTimer {
                        node: dev("absent"),
                        mac: PC1,
                        entry_epoch: index as u64,
                    },
                )
                .unwrap();
        }

        let active_start = (case.passive_timers as u64) + 1;
        for index in 0..case.active_frames {
            engine
                .schedule(
                    active_start + (index as u64 * 10_000),
                    InternalEvent::TransmitFrame {
                        from: ep("pc1", "eth0"),
                        frame: benchmark_frame(),
                    },
                )
                .unwrap();
        }

        let queue_live_bytes = test_allocation::live_bytes() - live_before_scheduling;
        (engine, queue_live_bytes)
    }

    fn run_scheduler_benchmark_case(case: SchedulerBenchmarkCase) -> (Duration, usize, usize) {
        let (mut engine, queue_live_bytes) = engine_with_benchmark_workload(case);
        let max_steps = case
            .passive_timers
            .saturating_add(case.active_frames.saturating_mul(32))
            .saturating_add(100);
        let started = Instant::now();
        let report = engine.run_until_quiescent(max_steps).unwrap();
        let elapsed = started.elapsed();

        assert_eq!(report.outcome, ExecutionOutcome::Quiescent);
        assert!(report.processed_events >= case.passive_timers + case.active_frames);
        black_box(engine);
        (elapsed, queue_live_bytes, report.processed_events)
    }

    fn assert_active_work_index_matches_scan(engine: &Engine) {
        let scanned_count = engine
            .scheduler
            .iter()
            .filter(|scheduled| engine.event_is_active(&scheduled.event))
            .count();
        assert_eq!(engine.active_scheduled_events, scanned_count);
        assert_eq!(engine.has_active_work(), scanned_count > 0);
    }

    #[test]
    #[ignore = "manual reproducible release benchmark; it reports timings instead of asserting them"]
    fn benchmark_run_until_quiescent_scheduler_scaling() {
        const REPEATS: usize = 7;
        let cases = [
            SchedulerBenchmarkCase {
                name: "active=1 passive=0",
                active_frames: 1,
                passive_timers: 0,
            },
            SchedulerBenchmarkCase {
                name: "active=1 passive=512",
                active_frames: 1,
                passive_timers: 512,
            },
            SchedulerBenchmarkCase {
                name: "active=1 passive=2048",
                active_frames: 1,
                passive_timers: 2_048,
            },
            SchedulerBenchmarkCase {
                name: "active=1 passive=8192",
                active_frames: 1,
                passive_timers: 8_192,
            },
            SchedulerBenchmarkCase {
                name: "active=64 passive=0",
                active_frames: 64,
                passive_timers: 0,
            },
            SchedulerBenchmarkCase {
                name: "active=64 passive=8192",
                active_frames: 64,
                passive_timers: 8_192,
            },
        ];

        println!(
            "scheduler benchmark: repeats={REPEATS}; scheduled_event_bytes={}",
            std::mem::size_of::<ScheduledEvent>()
        );
        for case in cases {
            let mut elapsed_samples = Vec::with_capacity(REPEATS);
            let mut queue_live_bytes = 0;
            let mut processed_events = 0;
            for _ in 0..REPEATS {
                let (elapsed, measured_queue_live_bytes, processed) =
                    run_scheduler_benchmark_case(case);
                elapsed_samples.push(elapsed);
                queue_live_bytes = measured_queue_live_bytes;
                processed_events = processed;
            }
            elapsed_samples.sort_unstable();
            let median = elapsed_samples[REPEATS / 2];
            println!(
                "{}; median_ns={}; queue_live_bytes={}; processed_events={}",
                case.name,
                median.as_nanos(),
                queue_live_bytes,
                processed_events
            );
        }
    }

    fn completed_rtt(snapshot: &TraceBufferSnapshot, sequence: u16) -> Option<u64> {
        snapshot
            .records
            .iter()
            .find_map(|record| match &record.event {
                ObservableEvent::PingCompleted {
                    sequence: found,
                    rtt_ns,
                    ..
                } if *found == sequence => Some(*rtt_ns),
                _ => None,
            })
    }

    #[test]
    fn e1_s1_successful_arp_is_ready_at_4300_ns() {
        let mut engine = base_engine();
        engine.send_ping(dev("pc1"), IP2, 1, 1).unwrap();
        let report = engine.advance_until(VirtualTime(4_300), 100).unwrap();
        assert_eq!(report.outcome, ExecutionOutcome::TargetTimeReached);
        assert_eq!(report.current_time, VirtualTime(4_300));
        assert_eq!(engine.arp_entry(&dev("pc1"), IP2), Some(PC2));
        assert_eq!(engine.pending_arp_packets(&dev("pc1"), IP2), 0);
        assert_eq!(engine.mac_table_len(&dev("sw1")), 2);
    }

    #[test]
    fn a1_external_send_ping_transmits_initial_arp_at_zero_ns() {
        let mut engine = base_engine();

        engine.send_ping(dev("pc1"), IP2, 1, 1).unwrap();
        engine.advance_until(VirtualTime(0), 10).unwrap();

        let snapshot = engine.get_trace_snapshot();
        assert!(snapshot.records.iter().any(|record| matches!(
            &record.event,
            ObservableEvent::ArpPacketQueued {
                at_device,
                target_ip,
                ..
            } if record.at == VirtualTime(0) && *at_device == dev("pc1") && *target_ip == IP2
        )));
        assert!(snapshot.records.iter().any(|record| matches!(
            &record.event,
            ObservableEvent::FrameTransmitted {
                link: transmitted_link,
                from,
                to,
                arrives_at,
                frame: EthernetFrame {
                    payload: EthernetPayload::Arp(ArpPacket {
                        operation: ArpOperation::Request,
                        ..
                    }),
                    ..
                },
            } if record.at == VirtualTime(0)
                && *transmitted_link == link("link1")
                && *from == ep("pc1", "eth0")
                && *to == ep("sw1", "port1")
                && *arrives_at == VirtualTime(1_000)
        )));
    }

    #[test]
    fn a1_deferred_transmit_does_not_use_an_admin_down_egress_interface() {
        let mut engine = base_engine();
        engine.send_ping(dev("pc1"), IP2, 1, 1).unwrap();
        engine.advance_until(VirtualTime(1_000), 100).unwrap();

        engine
            .set_interface_admin_state(ep("sw1", "port2"), false)
            .unwrap();
        engine.advance_until(VirtualTime(2_100), 100).unwrap();

        assert!(
            !engine
                .get_trace_snapshot()
                .records
                .iter()
                .any(|record| matches!(
                    &record.event,
                    ObservableEvent::FrameTransmitted { from, .. }
                        if record.at == VirtualTime(1_100) && *from == ep("sw1", "port2")
                ))
        );
        assert!(
            !engine
                .get_trace_snapshot()
                .records
                .iter()
                .any(|record| matches!(
                    &record.event,
                    ObservableEvent::FrameDelivered { at_endpoint, .. }
                        if record.at == VirtualTime(2_100) && *at_endpoint == ep("pc2", "eth0")
                ))
        );
        assert_eq!(
            engine
                .get_trace_snapshot()
                .records
                .iter()
                .filter(|record| matches!(
                    &record.event,
                    ObservableEvent::PacketDropped {
                        at_device,
                        at_interface: Some(interface),
                        pdu: DroppedPdu::Ethernet(_),
                        reason: DropReason::EgressInterfaceDown,
                    } if record.at == VirtualTime(1_100)
                        && *at_device == dev("sw1")
                        && *interface == iface("port2")
                ))
                .count(),
            1
        );
    }

    #[test]
    fn a1_deferred_transmit_records_unavailable_current_egress_link() {
        let mut engine = base_engine();
        engine.send_ping(dev("pc1"), IP2, 1, 1).unwrap();
        engine.advance_until(VirtualTime(1_000), 100).unwrap();

        engine.disconnect_link(&link("link2")).unwrap();
        engine.advance_until(VirtualTime(2_100), 100).unwrap();

        let snapshot = engine.get_trace_snapshot();
        assert!(!snapshot.records.iter().any(|record| matches!(
            &record.event,
            ObservableEvent::FrameTransmitted { from, .. }
                if record.at == VirtualTime(1_100) && *from == ep("sw1", "port2")
        )));
        assert!(!snapshot.records.iter().any(|record| matches!(
            &record.event,
            ObservableEvent::FrameDelivered { at_endpoint, .. }
                if record.at == VirtualTime(2_100) && *at_endpoint == ep("pc2", "eth0")
        )));
        assert_eq!(
            snapshot
                .records
                .iter()
                .filter(|record| matches!(
                    &record.event,
                    ObservableEvent::PacketDropped {
                        at_device,
                        at_interface: Some(interface),
                        pdu: DroppedPdu::Ethernet(_),
                        reason: DropReason::EgressLinkUnavailable,
                    } if record.at == VirtualTime(1_100)
                        && *at_device == dev("sw1")
                        && *interface == iface("port2")
                ))
                .count(),
            1
        );
    }

    #[test]
    fn a1_deferred_transmit_rechecks_the_current_reconnected_link() {
        let mut engine = base_engine();
        engine.send_ping(dev("pc1"), IP2, 1, 1).unwrap();
        engine.advance_until(VirtualTime(1_000), 100).unwrap();

        engine.disconnect_link(&link("link2")).unwrap();
        engine
            .connect_link(link("link2-new"), ep("pc2", "eth0"), ep("sw1", "port2"))
            .unwrap();
        engine.advance_until(VirtualTime(2_100), 100).unwrap();

        let snapshot = engine.get_trace_snapshot();
        assert!(!snapshot.records.iter().any(|record| matches!(
            &record.event,
            ObservableEvent::FrameTransmitted { link: transmitted_link, .. }
                if record.at == VirtualTime(1_100) && *transmitted_link == link("link2")
        )));
        assert!(!snapshot.records.iter().any(|record| matches!(
            &record.event,
            ObservableEvent::PacketDropped {
                at_device,
                at_interface: Some(interface),
                ..
            } if record.at == VirtualTime(1_100)
                && *at_device == dev("sw1")
                && *interface == iface("port2")
        )));
        assert!(snapshot.records.iter().any(|record| matches!(
            &record.event,
            ObservableEvent::FrameTransmitted {
                link: transmitted_link,
                from,
                to,
                arrives_at,
                ..
            } if record.at == VirtualTime(1_100)
                && *transmitted_link == link("link2-new")
                && *from == ep("sw1", "port2")
                && *to == ep("pc2", "eth0")
                && *arrives_at == VirtualTime(2_100)
        )));
        assert!(snapshot.records.iter().any(|record| matches!(
            &record.event,
            ObservableEvent::FrameDelivered {
                link: delivered_link,
                at_endpoint,
                ..
            } if record.at == VirtualTime(2_100)
                && *delivered_link == link("link2-new")
                && *at_endpoint == ep("pc2", "eth0")
        )));
    }

    #[test]
    fn e1_s2_cold_and_warm_ping_have_normative_rtts_and_obsolete_timer_is_quiescent() {
        let mut engine = base_engine();
        engine.send_ping(dev("pc1"), IP2, 1, 1).unwrap();
        let cold = engine.run_until_quiescent(100).unwrap();
        assert_eq!(cold.outcome, ExecutionOutcome::Quiescent);
        assert_eq!(cold.current_time, VirtualTime(8_700));
        assert_eq!(completed_rtt(&engine.get_trace_snapshot(), 1), Some(8_700));
        engine.advance_until(VirtualTime(10_000), 100).unwrap();
        engine.send_ping(dev("pc1"), IP2, 1, 2).unwrap();
        let warm = engine.run_until_quiescent(100).unwrap();
        assert_eq!(warm.outcome, ExecutionOutcome::Quiescent);
        assert_eq!(warm.current_time, VirtualTime(14_300));
        assert_eq!(completed_rtt(&engine.get_trace_snapshot(), 2), Some(4_300));
        assert!(!engine.get_trace_snapshot().records.iter().any(|r| matches!(&r.event, ObservableEvent::FrameDelivered { at_endpoint, .. } if at_endpoint.device == dev("pc3") && r.at.0 >= 10_000)));
    }

    #[test]
    fn active_work_index_ignores_passive_and_obsolete_timers() {
        let mut engine = base_engine();
        for index in 0..32 {
            engine
                .schedule(
                    (index as u64) + 1,
                    InternalEvent::MacEntryExpireTimer {
                        node: dev("absent"),
                        mac: PC1,
                        entry_epoch: index as u64,
                    },
                )
                .unwrap();
        }
        assert_eq!(engine.active_scheduled_events, 0);

        engine.send_ping(dev("pc1"), IP2, 1, 1).unwrap();
        assert!(engine.active_scheduled_events > 0);

        let report = engine.run_until_quiescent(200).unwrap();

        assert_eq!(report.outcome, ExecutionOutcome::Quiescent);
        assert_eq!(report.current_time, VirtualTime(8_700));
        assert_eq!(engine.active_scheduled_events, 0);
        assert!(!engine.scheduler.is_empty());
        assert!(!engine.has_active_work());
    }

    #[test]
    fn active_work_index_matches_the_scan_during_arp_resolution() {
        let mut engine = base_engine();
        for index in 0..32 {
            engine
                .schedule(
                    (index as u64) + 1,
                    InternalEvent::MacEntryExpireTimer {
                        node: dev("absent"),
                        mac: PC1,
                        entry_epoch: index as u64,
                    },
                )
                .unwrap();
        }
        engine.send_ping(dev("pc1"), IP2, 1, 1).unwrap();

        for _ in 0..200 {
            assert_active_work_index_matches_scan(&engine);
            if !engine.has_active_work() {
                break;
            }
            engine.process_one().unwrap();
        }

        assert_active_work_index_matches_scan(&engine);
        assert!(!engine.has_active_work());
        assert_eq!(engine.now(), VirtualTime(8_700));
        assert!(!engine.scheduler.is_empty());
    }

    #[test]
    fn e1_s3a_arp_retries_fail_at_three_seconds_and_empty_queue() {
        let mut engine = base_engine();
        engine
            .set_interface_admin_state(ep("pc2", "eth0"), false)
            .unwrap();
        engine.send_ping(dev("pc1"), IP2, 1, 1).unwrap();
        let report = engine.run_until_quiescent(500).unwrap();
        assert_eq!(report.outcome, ExecutionOutcome::Quiescent);
        assert_eq!(report.current_time, VirtualTime(3_000_000_000));
        assert_eq!(engine.pending_arp_packets(&dev("pc1"), IP2), 0);
        assert!(engine.get_trace_snapshot().records.iter().any(|r| matches!(&r.event, ObservableEvent::PingFailed { reason: PingFailureReason::ArpTimeout, .. } if r.at == VirtualTime(3_000_000_000))));
    }

    #[test]
    fn e1_s3b_reconnected_endpoint_does_not_receive_frame_in_old_link() {
        let mut engine = base_engine();
        engine.send_ping(dev("pc1"), IP2, 1, 1).unwrap();
        engine.advance_until(VirtualTime(500), 10).unwrap();
        engine.disconnect_link(&link("link1")).unwrap();
        engine
            .connect_link(link("link1_new"), ep("pc1", "eth0"), ep("sw1", "port1"))
            .unwrap();
        engine.advance_until(VirtualTime(1_500), 10).unwrap();
        assert!(engine.get_trace_snapshot().records.iter().any(|r| matches!(&r.event, ObservableEvent::PacketDropped { reason: DropReason::LinkDisconnectedInFlight, .. } if r.at == VirtualTime(1_000))));
    }

    #[test]
    fn e1_s4_switch_learns_migrated_port_without_duplicate_entry() {
        let mut engine = base_engine();
        engine.send_ping(dev("pc1"), IP2, 1, 1).unwrap();
        assert_eq!(
            engine.run_until_quiescent(100).unwrap().current_time,
            VirtualTime(8_700)
        );
        engine.advance_until(VirtualTime(20_000), 100).unwrap();
        engine.disconnect_link(&link("link2")).unwrap();
        engine
            .connect_link(link("link4"), ep("pc2", "eth0"), ep("sw1", "port4"))
            .unwrap();
        engine.send_ping(dev("pc2"), IP1, 2, 1).unwrap();
        let report = engine.run_until_quiescent(100).unwrap();
        assert_eq!(report.current_time, VirtualTime(24_300));
        assert_eq!(engine.mac_entry(&dev("sw1"), PC2), Some(iface("port4")));
        assert_eq!(engine.mac_table_len(&dev("sw1")), 2);
        assert_eq!(completed_rtt(&engine.get_trace_snapshot(), 1), Some(8_700));
        assert!(engine.get_trace_snapshot().records.iter().any(|r| matches!(&r.event, ObservableEvent::PingCompleted { host, identifier: 2, rtt_ns: 4_300, .. } if host == &dev("pc2") && r.at == VirtualTime(24_300))));
    }

    #[test]
    fn e1_s5_arp_entry_expires_without_use_renewal_and_requires_new_request() {
        let mut engine = base_engine();
        engine.send_ping(dev("pc1"), IP2, 1, 1).unwrap();
        engine.run_until_quiescent(100).unwrap();
        engine.advance_until(VirtualTime(10_000), 100).unwrap();
        engine.send_ping(dev("pc1"), IP2, 1, 2).unwrap();
        engine.run_until_quiescent(100).unwrap();
        engine
            .advance_until(VirtualTime(60_000_004_299), 100)
            .unwrap();
        assert_eq!(engine.arp_entry(&dev("pc1"), IP2), Some(PC2));
        engine
            .advance_until(VirtualTime(60_000_004_300), 100)
            .unwrap();
        assert_eq!(engine.arp_entry(&dev("pc1"), IP2), None);
        assert!(engine.get_trace_snapshot().records.iter().any(|r| matches!(&r.event, ObservableEvent::TableUpdated { at_device, update: TableUpdateDetail::ArpEntryExpired { ip } } if at_device == &dev("pc1") && *ip == IP2 && r.at == VirtualTime(60_000_004_300))));
        engine
            .advance_until(VirtualTime(61_000_000_000), 100)
            .unwrap();
        let before = engine.get_trace_snapshot().total_emitted_ever;
        engine.send_ping(dev("pc1"), IP2, 1, 3).unwrap();
        engine.run_until_quiescent(100).unwrap();
        let after = engine.get_trace_snapshot();
        assert!(after.records.iter().any(|r| r.seq.0 > before && matches!(&r.event, ObservableEvent::FrameTransmitted { frame: EthernetFrame { payload: EthernetPayload::Arp(ArpPacket { operation: ArpOperation::Request, .. }), .. }, from, .. } if from.device == dev("pc1"))));
    }

    #[test]
    fn e1_s6_loop_respects_step_budget_and_trace_capacity() {
        let mut engine = Engine::new(500).unwrap();
        engine.create_host(dev("pc1"), vec![iface("eth0")]).unwrap();
        engine
            .create_l2_switch(
                dev("sw1"),
                vec![
                    iface("port1"),
                    iface("port2"),
                    iface("port3"),
                    iface("port4"),
                ],
            )
            .unwrap();
        engine
            .create_l2_switch(
                dev("sw2"),
                vec![
                    iface("port1"),
                    iface("port2"),
                    iface("port3"),
                    iface("port4"),
                ],
            )
            .unwrap();
        engine
            .configure_interface(ep("pc1", "eth0"), PC1, Some((IP1, 24)), true)
            .unwrap();
        for switch in ["sw1", "sw2"] {
            for port in ["port1", "port2", "port3", "port4"] {
                engine
                    .configure_interface(ep(switch, port), MacAddress::ZERO, None, true)
                    .unwrap();
            }
        }
        engine
            .connect_link(link("link1"), ep("pc1", "eth0"), ep("sw1", "port1"))
            .unwrap();
        engine
            .connect_link(link("link_loop_a"), ep("sw1", "port2"), ep("sw2", "port2"))
            .unwrap();
        engine
            .connect_link(link("link_loop_b"), ep("sw1", "port3"), ep("sw2", "port3"))
            .unwrap();
        engine
            .send_ping(dev("pc1"), Ipv4Address([192, 168, 1, 99]), 9, 1)
            .unwrap();
        let report = engine.step_events(1_000).unwrap();
        assert_eq!(report.outcome, ExecutionOutcome::BudgetExhausted);
        assert_eq!(report.processed_events, 1_000);
        let trace = engine.get_trace_snapshot();
        assert_eq!(trace.records.len(), 500);
        assert!(trace.dropped_by_capacity > 0);
        assert_eq!(trace.dropped_by_capacity, trace.total_emitted_ever - 500);
        assert_eq!(
            trace.oldest_available_seq,
            Some(TraceSeq(trace.dropped_by_capacity + 1))
        );
        assert_eq!(
            trace.newest_available_seq,
            Some(TraceSeq(trace.total_emitted_ever))
        );
    }

    #[test]
    fn e1_a6_advance_without_events_and_past_target_are_explicit() {
        let mut engine = Engine::with_default_trace_capacity();
        assert_eq!(
            engine.advance_until(VirtualTime(77), 0).unwrap().outcome,
            ExecutionOutcome::TargetTimeReached
        );
        assert_eq!(engine.now(), VirtualTime(77));
        assert!(matches!(
            engine.advance_until(VirtualTime(76), 1),
            Err(EngineError::TargetTimeInPast { .. })
        ));
    }

    #[test]
    fn e1_a6_overflow_and_cancellation_do_not_panic_or_wrap() {
        let mut engine = Engine::with_default_trace_capacity();
        engine.now = VirtualTime(u64::MAX);
        assert_eq!(
            engine.schedule_after(
                1,
                InternalEvent::MacEntryExpireTimer {
                    node: dev("none"),
                    mac: PC1,
                    entry_epoch: 1
                }
            ),
            Err(EngineError::TimeOverflow)
        );
        engine.next_event_sequence = u64::MAX;
        assert_eq!(
            engine.schedule(
                0,
                InternalEvent::MacEntryExpireTimer {
                    node: dev("none"),
                    mac: PC1,
                    entry_epoch: 1
                }
            ),
            Err(EngineError::SequenceOverflow)
        );
        let mut engine = base_engine();
        engine.send_ping(dev("pc1"), IP2, 1, 1).unwrap();
        engine.request_cancellation();
        assert_eq!(engine.run_until_quiescent(100), Err(EngineError::Cancelled));
        engine.clear_cancellation();
        assert_eq!(
            engine.run_until_quiescent(100).unwrap().current_time,
            VirtualTime(8_700)
        );
    }

    #[test]
    fn canonical_reports_are_deterministic_over_one_hundred_runs() {
        let mut expected = None;
        for _ in 0..100 {
            let mut engine = base_engine();
            engine.send_ping(dev("pc1"), IP2, 1, 1).unwrap();
            let report = engine.run_until_quiescent(100).unwrap();
            let json = ScenarioCanonicalReport::from_execution(
                "E1-S2-cold",
                &report,
                engine.get_trace_snapshot(),
            )
            .to_canonical_json();
            let digest = sha256(json.as_bytes());
            if let Some(value) = &expected {
                assert_eq!(&digest, value);
            } else {
                expected = Some(digest);
            }
        }
    }

    #[test]
    fn scheduler_enforces_zero_delay_and_queue_capacity_guards() {
        let mut zero_delay = Engine::with_default_trace_capacity();
        for _ in 0..=MAX_ZERO_DELAY_EVENTS {
            zero_delay
                .schedule(
                    0,
                    InternalEvent::MacEntryExpireTimer {
                        node: dev("absent"),
                        mac: PC1,
                        entry_epoch: 1,
                    },
                )
                .unwrap();
        }
        let report = zero_delay.step_events(MAX_ZERO_DELAY_EVENTS + 1).unwrap();
        assert_eq!(report.processed_events, MAX_ZERO_DELAY_EVENTS);
        assert_eq!(
            report.outcome,
            ExecutionOutcome::HaltedBySafetyGuard(SafetyHaltReason::ZeroDelayLoop {
                at: VirtualTime(0),
                events_at_same_time: MAX_ZERO_DELAY_EVENTS + 1,
            })
        );

        let mut capacity = Engine::with_default_trace_capacity();
        for _ in 0..MAX_PENDING_QUEUE_SIZE {
            capacity
                .schedule(
                    1,
                    InternalEvent::MacEntryExpireTimer {
                        node: dev("absent"),
                        mac: PC1,
                        entry_epoch: 1,
                    },
                )
                .unwrap();
        }
        assert_eq!(capacity.active_scheduled_events, 0);
        assert_eq!(
            capacity.schedule(
                1,
                InternalEvent::TransmitFrame {
                    from: ep("absent", "eth0"),
                    frame: benchmark_frame(),
                },
            ),
            Err(EngineError::SafetyGuard(
                SafetyHaltReason::QueueCapacityExceeded {
                    pending_count: MAX_PENDING_QUEUE_SIZE + 1,
                    limit: MAX_PENDING_QUEUE_SIZE,
                }
            ))
        );
        assert_eq!(capacity.active_scheduled_events, 0);
    }

    #[test]
    fn deterministic_hash_uses_the_sha256_standard_vector_for_abc() {
        assert_eq!(
            sha256(b"abc"),
            [
                0xba, 0x78, 0x16, 0xbf, 0x8f, 0x01, 0xcf, 0xea, 0x41, 0x41, 0x40, 0xde, 0x5d, 0xae,
                0x22, 0x23, 0xb0, 0x03, 0x61, 0xa3, 0x96, 0x17, 0x7a, 0x9c, 0xb4, 0x10, 0xff, 0x61,
                0xf2, 0x00, 0x15, 0xad,
            ]
        );
    }

    fn sha256(input: &[u8]) -> [u8; 32] {
        const INITIAL: [u32; 8] = [
            0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
            0x5be0cd19,
        ];
        const K: [u32; 64] = [
            0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
            0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
            0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
            0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
            0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
            0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
            0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
            0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
            0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
            0xc67178f2,
        ];
        let bit_length = (input.len() as u64).checked_mul(8).unwrap();
        let mut bytes = input.to_vec();
        bytes.push(0x80);
        while bytes.len() % 64 != 56 {
            bytes.push(0);
        }
        bytes.extend_from_slice(&bit_length.to_be_bytes());
        let mut state = INITIAL;
        for block in bytes.chunks_exact(64) {
            let mut words = [0u32; 64];
            for (index, word) in words.iter_mut().take(16).enumerate() {
                *word = u32::from_be_bytes(block[index * 4..index * 4 + 4].try_into().unwrap());
            }
            for index in 16..64 {
                let s0 = words[index - 15].rotate_right(7)
                    ^ words[index - 15].rotate_right(18)
                    ^ (words[index - 15] >> 3);
                let s1 = words[index - 2].rotate_right(17)
                    ^ words[index - 2].rotate_right(19)
                    ^ (words[index - 2] >> 10);
                words[index] = words[index - 16]
                    .wrapping_add(s0)
                    .wrapping_add(words[index - 7])
                    .wrapping_add(s1);
            }
            let mut working = state;
            for (index, constant) in K.iter().enumerate() {
                let s1 = working[4].rotate_right(6)
                    ^ working[4].rotate_right(11)
                    ^ working[4].rotate_right(25);
                let choice = (working[4] & working[5]) ^ (!working[4] & working[6]);
                let temporary_one = working[7]
                    .wrapping_add(s1)
                    .wrapping_add(choice)
                    .wrapping_add(*constant)
                    .wrapping_add(words[index]);
                let s0 = working[0].rotate_right(2)
                    ^ working[0].rotate_right(13)
                    ^ working[0].rotate_right(22);
                let majority = (working[0] & working[1])
                    ^ (working[0] & working[2])
                    ^ (working[1] & working[2]);
                let temporary_two = s0.wrapping_add(majority);
                working = [
                    working[0].wrapping_add(temporary_one),
                    working[0],
                    working[1],
                    working[2],
                    working[3].wrapping_add(temporary_one),
                    working[4],
                    working[5],
                    working[6],
                ];
                working[0] = temporary_one.wrapping_add(temporary_two);
            }
            for (value, result) in state.iter_mut().zip(working) {
                *value = value.wrapping_add(result);
            }
        }
        let mut output = [0u8; 32];
        for (index, word) in state.iter().enumerate() {
            output[index * 4..index * 4 + 4].copy_from_slice(&word.to_be_bytes());
        }
        output
    }
}
