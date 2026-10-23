using PacketLens.SpikeE1.Domain;

namespace PacketLens.SpikeE1.Application;

public sealed partial class SimulationEngine
{
    public const ulong LinkPropagationDelayNs = 1_000;
    public const ulong DeviceProcessingDelayNs = 100;
    public const ulong ArpRequestTimeoutNs = 1_000_000_000;
    public const ulong ArpCacheTtlNs = 60_000_000_000;
    public const ulong MacTableTtlNs = 300_000_000_000;
    public const int MaxZeroDelayEvents = 1_000;
    public const int MaxPendingQueueSize = 100_000;

    private ulong _currentTimeNs;
    private ulong _nextSequence;
    private ulong _nextLinkInstance;
    private ulong _nextEpoch;
    private readonly Dictionary<string, DeviceState> _devices = new(StringComparer.Ordinal);
    private readonly Dictionary<string, LinkState> _links = new(StringComparer.Ordinal);
    private readonly Dictionary<ulong, string> _linkNamesByInstance = [];
    private readonly PriorityQueue<SimulationEvent, EventOrder> _events = new();
    private readonly TraceRingBuffer _trace;
    private readonly Dictionary<PingKey, ulong> _pingStarts = [];
    private bool _cancellationRequested;
    private SafetyHaltReason? _pendingSafetyHalt;
    private ulong? _lastProcessedEventTimeNs;
    private int _eventsAtSameTime;

    public SimulationEngine(int traceCapacity = 10_000)
    {
        if (traceCapacity <= 0) throw new ArgumentOutOfRangeException(nameof(traceCapacity));
        _trace = new TraceRingBuffer(traceCapacity);
    }

    public ulong CurrentTimeNs => _currentTimeNs;
    public TraceSnapshot Trace => _trace.Snapshot();

    public void RequestCancellation() => _cancellationRequested = true;
    public void ClearCancellation() => _cancellationRequested = false;

    public void CreateHost(string device, IReadOnlyList<string> interfaces) => CreateDevice(device, interfaces, isHost: true);
    public void CreateL2Switch(string device, IReadOnlyList<string> ports) => CreateDevice(device, ports, isHost: false);

    public void ConfigureInterface(Endpoint endpoint, MacAddress mac, Ipv4Address? address, byte prefixLength, bool adminUp)
    {
        if (prefixLength > 32) throw new ArgumentOutOfRangeException(nameof(prefixLength));
        var port = FindInterface(endpoint);
        if (_devices[endpoint.Device].Router is not null)
        {
            ConfigureRouterPort(endpoint,mac,address,prefixLength,adminUp);
            return;
        }
        HostIpv4Settings? revised = null;
        if (_devices[endpoint.Device].ConfiguredIpv4 is { } configured)
        {
            EnsureIpv4ConfigurationIdle();
            if (configured.Interface == endpoint.InterfaceName)
            {
                if (address is null) throw new ArgumentException("The configured IPv4 host requires an address.",nameof(address));
                revised = new HostIpv4Settings(new Ipv4InterfaceAddress(address.Value,prefixLength),configured.Settings.DefaultGateway);
            }
            else if (address is not null) throw new ArgumentException("An E2.1 host has one selected IPv4 interface.",nameof(address));
        }
        port.Mac = mac;
        port.Ipv4 = address is null ? null : new Ipv4Configuration(address.Value, prefixLength);
        port.AdminUp = adminUp;
        if (revised is not null) _devices[endpoint.Device].ConfiguredIpv4 = (endpoint.InterfaceName,revised);
    }

    public void ConfigureHostIpv4(Endpoint endpoint, MacAddress mac, HostIpv4Settings settings, bool adminUp)
    {
        ArgumentNullException.ThrowIfNull(settings);
        EnsureIpv4ConfigurationIdle();
        var port = FindInterface(endpoint);
        var host = _devices[endpoint.Device];
        if (!host.IsHost || host.Interfaces.Any(pair => pair.Key != endpoint.InterfaceName && pair.Value.Ipv4 is not null))
            throw new ArgumentException("An E2.1 host has one selected IPv4 interface.",nameof(endpoint));
        port.Mac = mac;
        port.Ipv4 = new Ipv4Configuration(settings.Interface.Address,(byte)settings.Interface.Network.Prefix.Value);
        port.AdminUp = adminUp;
        host.ConfiguredIpv4 = (endpoint.InterfaceName,settings);
    }

    public HostIpv4Settings? HostIpv4Configuration(string host) =>
        _devices.TryGetValue(host,out var device) ? device.ConfiguredIpv4?.Settings : null;

    private void EnsureIpv4ConfigurationIdle()
    {
        if (HasActiveWork() || _pingStarts.Count != 0) throw new SimulationException(SimulationErrorCode.EngineBusy);
    }

    public void ConnectLink(string link, Endpoint a, Endpoint b)
    {
        if (_links.ContainsKey(link)) throw new ArgumentException("Duplicate link.", nameof(link));
        if (a == b) throw new ArgumentException("A link requires two distinct interfaces.",nameof(b));
        var first = FindInterface(a);
        var second = FindInterface(b);
        if (first.LinkInstance is not null || second.LinkInstance is not null) throw new InvalidOperationException("An endpoint already has a link.");

        _nextLinkInstance = checked(_nextLinkInstance + 1);
        first.LinkInstance = _nextLinkInstance;
        second.LinkInstance = _nextLinkInstance;
        _links.Add(link, new LinkState(_nextLinkInstance, a, b));
        _linkNamesByInstance.Add(_nextLinkInstance, link);
    }

    public void DisconnectLink(string link)
    {
        if (!_links.Remove(link, out var removed)) throw new ArgumentException("Unknown link.", nameof(link));
        _linkNamesByInstance.Remove(removed.Instance);
        var first = FindInterface(removed.A);
        var second = FindInterface(removed.B);
        if (first.LinkInstance == removed.Instance) first.LinkInstance = null;
        if (second.LinkInstance == removed.Instance) second.LinkInstance = null;
    }

    public void SetInterfaceAdminState(Endpoint endpoint, bool adminUp) => FindInterface(endpoint).AdminUp = adminUp;

    public void SendPing(string source, Ipv4Address targetIp, ushort identifier, ushort sequence)
    {
        ValidateIpv4Topology();
        var (_, port) = FindHostPortFor(source, targetIp);
        if (_devices[source].ConfiguredIpv4 is { } configured && !configured.Settings.Interface.Network.Contains(targetIp) && configured.Settings.DefaultGateway is null)
            throw new InvalidOperationException("No host gateway for remote destination.");
        _pingStarts.Add(new PingKey(source, targetIp, identifier, sequence), _currentTimeNs);
        var packet = new Ipv4Packet(port.Ipv4!.Address, targetIp, 64, new IcmpPacket(IcmpKind.EchoRequest, identifier, sequence));
        SendIpv4(source, targetIp, packet, _currentTimeNs);
    }

    public ExecutionReport AdvanceUntil(ulong targetTimeNs, int maxStepsGuard)
    {
        ArgumentOutOfRangeException.ThrowIfNegative(maxStepsGuard);
        if (targetTimeNs < _currentTimeNs) throw new SimulationException(SimulationErrorCode.TargetTimeInPast);

        var processed = 0;
        while (_events.TryPeek(out _, out var order) && order.AtNs <= targetTimeNs)
        {
            ThrowIfCancelled();
            if (TryGetSafetyHalt(processed, out var safetyHalt)) return safetyHalt;
            if (processed == maxStepsGuard) return new ExecutionReport(ExecutionOutcome.BudgetExhausted, processed, _currentTimeNs);
            if (TryGetZeroDelayHalt(order, processed, out safetyHalt)) return safetyHalt;
            var simulationEvent = _events.Dequeue();
            ProcessEvent(order, simulationEvent);
            processed++;
        }

        _currentTimeNs = targetTimeNs;
        return new ExecutionReport(ExecutionOutcome.TargetTimeReached, processed, _currentTimeNs);
    }

    public ExecutionReport StepEvents(int maxSteps)
    {
        ArgumentOutOfRangeException.ThrowIfNegative(maxSteps);
        var processed = 0;
        while (processed < maxSteps && _events.TryPeek(out _, out var order))
        {
            ThrowIfCancelled();
            if (TryGetSafetyHalt(processed, out var safetyHalt)) return safetyHalt;
            if (TryGetZeroDelayHalt(order, processed, out safetyHalt)) return safetyHalt;
            var simulationEvent = _events.Dequeue();
            ProcessEvent(order, simulationEvent);
            processed++;
        }

        return new ExecutionReport(_events.Count == 0 ? ExecutionOutcome.Idle : ExecutionOutcome.BudgetExhausted, processed, _currentTimeNs);
    }

    public ExecutionReport RunUntilQuiescent(int maxStepsGuard)
    {
        ArgumentOutOfRangeException.ThrowIfNegative(maxStepsGuard);
        var processed = 0;
        if (TryGetSafetyHalt(processed, out var safetyHalt)) return safetyHalt;
        while (HasActiveWork())
        {
            ThrowIfCancelled();
            if (TryGetSafetyHalt(processed, out safetyHalt)) return safetyHalt;
            if (processed == maxStepsGuard)
            {
                return new ExecutionReport(ExecutionOutcome.BudgetExhausted, processed, _currentTimeNs);
            }

            _events.TryPeek(out _, out var order);
            if (TryGetZeroDelayHalt(order, processed, out safetyHalt)) return safetyHalt;
            var simulationEvent = _events.Dequeue();
            ProcessEvent(order, simulationEvent);
            processed++;
        }

        return new ExecutionReport(ExecutionOutcome.Quiescent, processed, _currentTimeNs);
    }

    public MacAddress? ArpEntry(string host, Ipv4Address ip) => _devices.TryGetValue(host, out var device) && device.ArpCache.TryGetValue(ip, out var entry) ? entry.Mac : null;
    public int PendingArpPacketCount(string host, Ipv4Address ip) => _devices.TryGetValue(host, out var device) && device.PendingArp.TryGetValue(ip, out var packets) ? packets.Count : 0;
    public int MacTableEntryCount(string device) => _devices.TryGetValue(device, out var node) ? node.MacTable.Count : 0;
    public string? MacPort(string device, MacAddress mac) => _devices.TryGetValue(device, out var node) && node.MacTable.TryGetValue(mac, out var entry) ? entry.Port : null;

    internal void ScheduleNoOperationForTesting(ulong atNs) => Schedule(atNs, new NoOperationEvent());
    internal void SetNextSequenceForTesting(ulong value) => _nextSequence = value;
    internal void InjectFrameForTesting(Endpoint destination,EthernetFrame frame,ulong at)
    {
        if (at < _currentTimeNs) throw new SimulationException(SimulationErrorCode.TargetTimeInPast);
        var instance=FindInterface(destination).LinkInstance ?? throw new ArgumentException("Injection requires a connected interface.",nameof(destination));
        Schedule(at,new FrameArrivalEvent(instance,destination,frame));
    }

    private void CreateDevice(string device, IReadOnlyList<string> portNames, bool isHost)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(device);
        if (portNames.Count == 0 || portNames.Distinct(StringComparer.Ordinal).Count() != portNames.Count) throw new ArgumentException("A device needs uniquely named ports.", nameof(portNames));
        if (!_devices.TryAdd(device, new DeviceState(isHost, portNames))) throw new ArgumentException("Duplicate device.", nameof(device));
    }

    private void SendIpv4(string source, Ipv4Address nextHop, Ipv4Packet packet, ulong transmitAtNs)
    {
        if (_devices[source].ConfiguredIpv4 is { } configured && !configured.Settings.Interface.Network.Contains(nextHop))
            nextHop = configured.Settings.DefaultGateway ?? throw new InvalidOperationException("No host gateway for remote destination.");
        var (endpoint, port) = FindHostPortFor(source, nextHop);
        var host = _devices[source];
        if (host.ArpCache.TryGetValue(nextHop, out var entry))
        {
            Schedule(transmitAtNs, new TransmitFrameEvent(endpoint, new EthernetFrame(port.Mac!.Value, entry.Mac, packet)));
            return;
        }

        var retryAtNs = AddTime(transmitAtNs, ArpRequestTimeoutNs);
        _trace.Add(new ArpPacketQueued(_currentTimeNs, source, nextHop, packet));
        if (!host.PendingArp.TryGetValue(nextHop, out var pending))
        {
            pending = [];
            host.PendingArp.Add(nextHop, pending);
        }

        pending.Add(packet);
        if (host.Resolutions.ContainsKey(nextHop)) return;

        var epoch = NextEpoch();
        host.Resolutions.Add(nextHop, new ArpResolution(1, epoch, endpoint.InterfaceName));
        ScheduleArpRequest(endpoint, port, nextHop, transmitAtNs);
        Schedule(retryAtNs, new ArpRetryEvent(source, nextHop, 1, epoch));
    }

    private void ScheduleArpRequest(Endpoint endpoint, InterfaceState port, Ipv4Address targetIp, ulong atNs)
    {
        var frame = new EthernetFrame(port.Mac!.Value, MacAddress.Broadcast, new ArpPacket(ArpOperation.Request, port.Mac.Value, port.Ipv4!.Address, MacAddress.Zero, targetIp));
        Schedule(atNs, new TransmitFrameEvent(endpoint, frame));
    }

    private void Schedule(ulong atNs, SimulationEvent simulationEvent)
    {
        if (_pendingSafetyHalt is not null) return;
        if (_events.Count >= MaxPendingQueueSize)
        {
            _pendingSafetyHalt = new QueueCapacityExceeded(_events.Count + 1, MaxPendingQueueSize);
            return;
        }

        _events.Enqueue(simulationEvent, new EventOrder(atNs, NextSequence()));
    }

    private bool TryGetSafetyHalt(int processed, out ExecutionReport report)
    {
        if (_pendingSafetyHalt is not null)
        {
            report = new ExecutionReport(ExecutionOutcome.HaltedBySafetyGuard, processed, _currentTimeNs, _pendingSafetyHalt);
            return true;
        }

        report = null!;
        return false;
    }

    private bool TryGetZeroDelayHalt(EventOrder order, int processed, out ExecutionReport report)
    {
        if (_lastProcessedEventTimeNs == order.AtNs && _eventsAtSameTime >= MaxZeroDelayEvents)
        {
            report = new ExecutionReport(ExecutionOutcome.HaltedBySafetyGuard, processed, _currentTimeNs, new ZeroDelayLoop(order.AtNs, _eventsAtSameTime + 1));
            return true;
        }

        report = null!;
        return false;
    }

    private void ProcessEvent(EventOrder order, SimulationEvent simulationEvent)
    {
        _currentTimeNs = order.AtNs;
        Handle(simulationEvent);
        if (_lastProcessedEventTimeNs == order.AtNs)
        {
            _eventsAtSameTime++;
        }
        else
        {
            _lastProcessedEventTimeNs = order.AtNs;
            _eventsAtSameTime = 1;
        }
    }

    private void Handle(SimulationEvent simulationEvent)
    {
        switch (simulationEvent)
        {
            case TransmitFrameEvent transmit: Transmit(transmit); break;
            case FrameArrivalEvent arrival: Deliver(arrival); break;
            case ArpRetryEvent retry: HandleArpRetry(retry); break;
            case ArpExpireEvent expiry: ExpireArp(expiry); break;
            case MacExpireEvent expiry: ExpireMac(expiry); break;
            case RouterArpExpireEvent expiry: ExpireRouterArp(expiry); break;
            case RouterArpRetryEvent retry: RetryRouterArp(retry); break;
            case NoOperationEvent: break;
            default: throw new InvalidOperationException("Unknown simulation event.");
        }
    }

    private void Transmit(TransmitFrameEvent transmit)
    {
        var port = FindInterface(transmit.From);
        if (!port.AdminUp)
        {
            DropEthernet(transmit.From, transmit.Frame, DropReason.EgressInterfaceDown);
            return;
        }
        if (port.LinkInstance is not ulong instance || !_linkNamesByInstance.TryGetValue(instance, out var linkName))
        {
            DropEthernet(transmit.From, transmit.Frame, DropReason.EgressLinkUnavailable);
            return;
        }
        var link = _links[linkName];
        var destination = link.A == transmit.From ? link.B : link.A;
        var arrivalTime = AddTime(_currentTimeNs, LinkPropagationDelayNs);
        _trace.Add(new FrameTransmitted(_currentTimeNs, linkName, transmit.From, destination, arrivalTime, transmit.Frame));
        Schedule(arrivalTime, new FrameArrivalEvent(instance, destination, transmit.Frame));
    }

    private void Deliver(FrameArrivalEvent arrival)
    {
        if (!_linkNamesByInstance.TryGetValue(arrival.LinkInstance, out var linkName))
        {
            DropEthernet(arrival.Destination, arrival.Frame, DropReason.LinkDisconnectedInFlight);
            return;
        }
        var destinationPort = FindInterface(arrival.Destination);
        if (destinationPort.LinkInstance != arrival.LinkInstance)
        {
            DropEthernet(arrival.Destination, arrival.Frame, DropReason.LinkDisconnectedInFlight);
            return;
        }

        if (!destinationPort.AdminUp)
        {
            DropEthernet(arrival.Destination, arrival.Frame, DropReason.IngressInterfaceDown);
            return;
        }

        _trace.Add(new FrameDelivered(_currentTimeNs, linkName, arrival.Destination, arrival.Frame));
        if (_devices[arrival.Destination.Device].IsHost) ReceiveAtHost(arrival.Destination, arrival.Frame);
        else if (_devices[arrival.Destination.Device].Router is not null) ReceiveAtRouter(arrival.Destination,arrival.Frame);
        else ReceiveAtSwitch(arrival.Destination, arrival.Frame);
    }

    private void ReceiveAtHost(Endpoint endpoint, EthernetFrame frame)
    {
        var port = FindInterface(endpoint);
        if (port.Ipv4 is null || port.Mac is null) return;
        if (frame.Payload is ArpPacket arp)
        {
            ReceiveArpAtHost(endpoint, port, frame, arp);
            return;
        }

        if (frame.Payload is Ipv4Packet ipv4 && ipv4.DestinationIp == port.Ipv4.Address)
        {
            ReceiveIpv4AtHost(endpoint, port, ipv4);
        }
    }

    private void ReceiveArpAtHost(Endpoint endpoint, InterfaceState port, EthernetFrame frame, ArpPacket arp)
    {
        var addressedToHost = arp.TargetIp == port.Ipv4!.Address && (arp.Operation == ArpOperation.Request || arp.TargetMac == port.Mac);
        if (!addressedToHost)
        {
            DropEthernet(endpoint, frame, DropReason.ArpTargetIpMismatch);
            return;
        }

        LearnArp(endpoint.Device, arp.SenderIp, arp.SenderMac);
        if (arp.Operation == ArpOperation.Request)
        {
            var reply = new EthernetFrame(port.Mac!.Value, arp.SenderMac, new ArpPacket(ArpOperation.Reply, port.Mac.Value, port.Ipv4.Address, arp.SenderMac, arp.SenderIp));
            Schedule(AddTime(_currentTimeNs, DeviceProcessingDelayNs), new TransmitFrameEvent(endpoint, reply));
            return;
        }

        var host = _devices[endpoint.Device];
        host.Resolutions.Remove(arp.SenderIp);
        if (!host.PendingArp.Remove(arp.SenderIp, out var pending)) return;
        foreach (var packet in pending) SendIpv4(endpoint.Device, arp.SenderIp, packet, AddTime(_currentTimeNs, DeviceProcessingDelayNs));
    }

    private void ReceiveIpv4AtHost(Endpoint endpoint, InterfaceState port, Ipv4Packet packet)
    {
        if (packet.Payload.Kind == IcmpKind.EchoRequest)
        {
            var reply = new Ipv4Packet(port.Ipv4!.Address, packet.SourceIp, 64, new IcmpPacket(IcmpKind.EchoReply, packet.Payload.Identifier, packet.Payload.Sequence));
            SendIpv4(endpoint.Device, packet.SourceIp, reply, AddTime(_currentTimeNs, DeviceProcessingDelayNs));
            return;
        }

        if (packet.Payload.Kind == IcmpKind.EchoReply && _pingStarts.Remove(new PingKey(endpoint.Device, packet.SourceIp, packet.Payload.Identifier, packet.Payload.Sequence), out var startedAt))
        {
            _trace.Add(new PingCompleted(_currentTimeNs, endpoint.Device, packet.SourceIp, packet.Payload.Identifier, packet.Payload.Sequence, checked(_currentTimeNs - startedAt)));
        }
    }

    private void ReceiveAtSwitch(Endpoint ingress, EthernetFrame frame)
    {
        var node = _devices[ingress.Device];
        LearnMac(ingress.Device, frame.SourceMac, ingress.InterfaceName);
        IEnumerable<string> egressPorts;
        if (frame.DestinationMac == MacAddress.Broadcast || !node.MacTable.TryGetValue(frame.DestinationMac, out var destination))
        {
            egressPorts = node.Interfaces.Where(pair => pair.Key != ingress.InterfaceName && pair.Value.AdminUp && pair.Value.LinkInstance is not null).Select(pair => pair.Key);
        }
        else
        {
            egressPorts = destination.Port == ingress.InterfaceName || !node.Interfaces[destination.Port].AdminUp || node.Interfaces[destination.Port].LinkInstance is null ? [] : [destination.Port];
        }

        var transmitAt = AddTime(_currentTimeNs, DeviceProcessingDelayNs);
        foreach (var port in egressPorts) Schedule(transmitAt, new TransmitFrameEvent(new Endpoint(ingress.Device, port), frame));
    }

    private void LearnArp(string host, Ipv4Address ip, MacAddress mac)
    {
        var expiresAt = AddTime(_currentTimeNs, ArpCacheTtlNs);
        var epoch = NextEpoch();
        _devices[host].ArpCache[ip] = new ArpCacheEntry(mac, epoch);
        _trace.Add(new TableUpdated(_currentTimeNs, host, new ArpLearned(ip, mac, expiresAt)));
        Schedule(expiresAt, new ArpExpireEvent(host, ip, epoch));
    }

    private void LearnMac(string device, MacAddress mac, string port)
    {
        var expiresAt = AddTime(_currentTimeNs, MacTableTtlNs);
        var epoch = NextEpoch();
        _devices[device].MacTable[mac] = new MacEntry(port, epoch);
        _trace.Add(new TableUpdated(_currentTimeNs, device, new MacLearned(mac, port, expiresAt)));
        Schedule(expiresAt, new MacExpireEvent(device, mac, epoch));
    }

    private void HandleArpRetry(ArpRetryEvent retry)
    {
        if (!_devices.TryGetValue(retry.Node, out var host) || !host.Resolutions.TryGetValue(retry.TargetIp, out var resolution) || resolution.Epoch != retry.ResolutionEpoch || resolution.Attempt != retry.Attempt)
        {
            return;
        }

        if (retry.Attempt < 3)
        {
            var nextAttempt = checked((byte)(retry.Attempt + 1));
            host.Resolutions[retry.TargetIp] = resolution with { Attempt = nextAttempt };
            var endpoint = new Endpoint(retry.Node, resolution.InterfaceName);
            var port = FindInterface(endpoint);
            if (port.AdminUp && port.LinkInstance is not null) ScheduleArpRequest(endpoint, port, retry.TargetIp, _currentTimeNs);
            Schedule(AddTime(_currentTimeNs, ArpRequestTimeoutNs), new ArpRetryEvent(retry.Node, retry.TargetIp, nextAttempt, retry.ResolutionEpoch));
            return;
        }

        host.Resolutions.Remove(retry.TargetIp);
        if (!host.PendingArp.Remove(retry.TargetIp, out var packets)) return;
        foreach (var packet in packets)
        {
            _trace.Add(new PacketDropped(_currentTimeNs, retry.Node, null, null, packet, DropReason.ArpResolutionFailed));
            if (packet.Payload.Kind == IcmpKind.EchoRequest && _pingStarts.Remove(new PingKey(retry.Node, packet.DestinationIp, packet.Payload.Identifier, packet.Payload.Sequence)))
            {
                _trace.Add(new PingFailed(_currentTimeNs, retry.Node, packet.DestinationIp, packet.Payload.Identifier, packet.Payload.Sequence, PingFailureReason.ArpTimeout));
            }
        }
    }

    private void ExpireArp(ArpExpireEvent expiry)
    {
        if (_devices.TryGetValue(expiry.Node, out var host) && host.ArpCache.TryGetValue(expiry.Ip, out var entry) && entry.Epoch == expiry.EntryEpoch)
        {
            host.ArpCache.Remove(expiry.Ip);
            _trace.Add(new TableUpdated(_currentTimeNs, expiry.Node, new ArpEntryExpired(expiry.Ip)));
        }
    }

    private void ExpireMac(MacExpireEvent expiry)
    {
        if (_devices.TryGetValue(expiry.Node, out var device) && device.MacTable.TryGetValue(expiry.Mac, out var entry) && entry.Epoch == expiry.EntryEpoch)
        {
            device.MacTable.Remove(expiry.Mac);
            _trace.Add(new TableUpdated(_currentTimeNs, expiry.Node, new MacEntryExpired(expiry.Mac)));
        }
    }

    private (Endpoint Endpoint, InterfaceState Port) FindHostPortFor(string device, Ipv4Address targetIp)
    {
        if (!_devices.TryGetValue(device, out var node) || !node.IsHost) throw new ArgumentException("The source must be an existing host.", nameof(device));
        if (node.ConfiguredIpv4 is { } configured)
        {
            var selected = node.Interfaces[configured.Interface];
            if (!selected.AdminUp) throw new InvalidOperationException("The selected IPv4 interface is down.");
            return (Endpoint.Of(device,configured.Interface),selected);
        }
        foreach (var pair in node.Interfaces)
        {
            if (pair.Value.AdminUp && pair.Value.Ipv4 is { } address && address.Address.IsInSameSubnet(targetIp, address.PrefixLength)) return (new Endpoint(device, pair.Key), pair.Value);
        }

        throw new InvalidOperationException("No route in the E1 local-subnet model.");
    }

    private InterfaceState FindInterface(Endpoint endpoint)
    {
        if (!_devices.TryGetValue(endpoint.Device, out var device) || !device.Interfaces.TryGetValue(endpoint.InterfaceName, out var port)) throw new ArgumentException("Unknown endpoint.", nameof(endpoint));
        return port;
    }

    private static ulong AddTime(ulong timeNs, ulong delayNs)
    {
        if (ulong.MaxValue - timeNs < delayNs) throw new SimulationException(SimulationErrorCode.TimeOverflow);
        return timeNs + delayNs;
    }

    private ulong NextSequence()
    {
        if (_nextSequence == ulong.MaxValue) throw new SimulationException(SimulationErrorCode.SequenceOverflow);
        return ++_nextSequence;
    }

    private ulong NextEpoch() => _nextEpoch = checked(_nextEpoch + 1);

    private void ThrowIfCancelled()
    {
        if (_cancellationRequested) throw new SimulationException(SimulationErrorCode.Cancelled);
    }

    private void DropEthernet(Endpoint endpoint, EthernetFrame frame, DropReason reason) => _trace.Add(new PacketDropped(_currentTimeNs, endpoint.Device, endpoint.InterfaceName, frame, null, reason));

    private bool HasActiveWork() => _events.UnorderedItems.Any(item => item.Element switch
    {
        TransmitFrameEvent or FrameArrivalEvent => true,
        RouterArpRetryEvent retry => RouterResolutionActive(retry),
        ArpRetryEvent retry => _devices.TryGetValue(retry.Node, out var host)
            && host.Resolutions.TryGetValue(retry.TargetIp, out var resolution)
            && resolution.Epoch == retry.ResolutionEpoch
            && resolution.Attempt == retry.Attempt,
        _ => false
    });

    private sealed class DeviceState(bool isHost, IReadOnlyList<string> names)
    {
        public Dictionary<(string Interface, Ipv4Address Ip), ArpCacheEntry> RouterArpCache { get; } = [];
        public Dictionary<(string Interface, Ipv4Address Ip), List<RouterPendingPacket>> RouterPending { get; } = [];
        public Dictionary<(string Interface, Ipv4Address Ip), ArpResolution> RouterResolutions { get; } = [];
        public RouterConfiguration? Router { get; set; }
        public (string Interface, HostIpv4Settings Settings)? ConfiguredIpv4 { get; set; }
        public bool IsHost { get; } = isHost;
        public SortedDictionary<string, InterfaceState> Interfaces { get; } = new(names.ToDictionary(name => name, _ => new InterfaceState(), StringComparer.Ordinal), StringComparer.Ordinal);
        public SortedDictionary<Ipv4Address, ArpCacheEntry> ArpCache { get; } = [];
        public SortedDictionary<Ipv4Address, List<Ipv4Packet>> PendingArp { get; } = [];
        public SortedDictionary<Ipv4Address, ArpResolution> Resolutions { get; } = [];
        public SortedDictionary<MacAddress, MacEntry> MacTable { get; } = [];
    }

    private sealed class InterfaceState
    {
        public Ipv4LinkMode Ipv4Mode { get; set; }
        public MacAddress? Mac { get; set; }
        public Ipv4Configuration? Ipv4 { get; set; }
        public bool AdminUp { get; set; }
        public ulong? LinkInstance { get; set; }
    }

    private sealed record LinkState(ulong Instance, Endpoint A, Endpoint B);
    private sealed record Ipv4Configuration(Ipv4Address Address, byte PrefixLength);
    private sealed record ArpCacheEntry(MacAddress Mac, ulong Epoch);
    private sealed record ArpResolution(byte Attempt, ulong Epoch, string InterfaceName);
    private sealed record MacEntry(string Port, ulong Epoch);
    private sealed record PingKey(string Source, Ipv4Address TargetIp, ushort Identifier, ushort Sequence);
    private abstract record SimulationEvent;
    private sealed record TransmitFrameEvent(Endpoint From, EthernetFrame Frame) : SimulationEvent;
    private sealed record FrameArrivalEvent(ulong LinkInstance, Endpoint Destination, EthernetFrame Frame) : SimulationEvent;
    private sealed record ArpRetryEvent(string Node, Ipv4Address TargetIp, byte Attempt, ulong ResolutionEpoch) : SimulationEvent;
    private sealed record ArpExpireEvent(string Node, Ipv4Address Ip, ulong EntryEpoch) : SimulationEvent;
    private sealed record MacExpireEvent(string Node, MacAddress Mac, ulong EntryEpoch) : SimulationEvent;
    private sealed record NoOperationEvent : SimulationEvent;
    private readonly record struct EventOrder(ulong AtNs, ulong Sequence) : IComparable<EventOrder>
    {
        public int CompareTo(EventOrder other) => AtNs != other.AtNs ? AtNs.CompareTo(other.AtNs) : Sequence.CompareTo(other.Sequence);
    }

    private sealed class TraceRingBuffer(int capacity)
    {
        private readonly Queue<TraceEvent> _records = new();
        private ulong _total;
        private ulong _dropped;

        public void Add(TraceEvent trace)
        {
            _total = checked(_total + 1);
            trace.TraceSequence = _total;
            if (_records.Count == capacity)
            {
                _records.Dequeue();
                _dropped = checked(_dropped + 1);
            }

            _records.Enqueue(trace);
        }

        public TraceSnapshot Snapshot() => new(_total, _dropped, _records.Count == 0 ? null : _dropped + 1, _records.Count == 0 ? null : _total, _records.ToArray());
    }
}

internal static class SimulationTestAccess
{
    internal static void ScheduleNoOperation(SimulationEngine engine, ulong atNs) => engine.ScheduleNoOperationForTesting(atNs);
    internal static void SetNextSequence(SimulationEngine engine, ulong value) => engine.SetNextSequenceForTesting(value);
}
