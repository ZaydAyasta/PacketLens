using PacketLens.SpikeE1.Domain;

namespace PacketLens.SpikeE1.Application;

public sealed partial class SimulationEngine
{
    private sealed record RouterArpExpireEvent(Endpoint Endpoint,Ipv4Address Ip,ulong Epoch) : SimulationEvent;
    private sealed record RouterArpRetryEvent(Endpoint Endpoint,Ipv4Address Target,byte Attempt,ulong Epoch) : SimulationEvent;
    private sealed record RouterPendingPacket(Ipv4Packet Packet,Endpoint? Ingress,EthernetFrame? Received);

    private void ReceiveAtRouter(Endpoint endpoint,EthernetFrame frame)
    {
        var port=FindInterface(endpoint);
        if (frame.DestinationMac != port.Mac && frame.DestinationMac != MacAddress.Broadcast)
        { DropEthernet(endpoint,frame,DropReason.DestinationMacMismatch); return; }
        if (frame.Payload is Ipv4Packet ipv4) { RouteReceivedIpv4(endpoint,frame,ipv4); return; }
        if (frame.Payload is not ArpPacket arp) return;
        if (arp.TargetIp != port.Ipv4!.Address || (arp.Operation==ArpOperation.Reply && arp.TargetMac != port.Mac))
        { DropEthernet(endpoint,frame,DropReason.ArpTargetIpMismatch); return; }
        var expires=AddTime(_currentTimeNs,ArpCacheTtlNs); var epoch=NextEpoch();
        _devices[endpoint.Device].RouterArpCache[(endpoint.InterfaceName,arp.SenderIp)]=new ArpCacheEntry(arp.SenderMac,epoch);
        _trace.Add(new TableUpdated(_currentTimeNs,endpoint.Device,new ArpLearned(arp.SenderIp,arp.SenderMac,expires)) { ObservationInterface=endpoint.InterfaceName });
        Schedule(expires,new RouterArpExpireEvent(endpoint,arp.SenderIp,epoch));
        if (arp.Operation==ArpOperation.Request)
        {
            var reply=new EthernetFrame(port.Mac!.Value,arp.SenderMac,new ArpPacket(ArpOperation.Reply,port.Mac.Value,port.Ipv4.Address,arp.SenderMac,arp.SenderIp));
            Schedule(AddTime(_currentTimeNs,DeviceProcessingDelayNs),new TransmitFrameEvent(endpoint,reply));
        }
        else
        {
            var router=_devices[endpoint.Device]; var key=(endpoint.InterfaceName,arp.SenderIp);
            router.RouterResolutions.Remove(key);
            if (router.RouterPending.Remove(key,out var pending))
                foreach (var item in pending) EmitRouterIpv4(endpoint,arp.SenderMac,item.Packet,AddTime(_currentTimeNs,DeviceProcessingDelayNs));
        }
    }

    private void ExpireRouterArp(RouterArpExpireEvent expiry)
    {
        var cache=_devices[expiry.Endpoint.Device].RouterArpCache; var key=(expiry.Endpoint.InterfaceName,expiry.Ip);
        if (!cache.TryGetValue(key,out var entry) || entry.Epoch != expiry.Epoch) return;
        cache.Remove(key);
        _trace.Add(new TableUpdated(_currentTimeNs,expiry.Endpoint.Device,new ArpEntryExpired(expiry.Ip)) { ObservationInterface=expiry.Endpoint.InterfaceName });
    }

    public MacAddress? RouterArpEntry(Endpoint endpoint, Ipv4Address ip)
    {
        _ = FindInterface(endpoint);
        _ = Router(endpoint.Device);
        return _devices[endpoint.Device].RouterArpCache.TryGetValue((endpoint.InterfaceName,ip),out var entry) ? entry.Mac : null;
    }

    private void RouteReceivedIpv4(Endpoint ingress,EthernetFrame received,Ipv4Packet packet)
    {
        var router=_devices[ingress.Device];
        bool OwnBroadcast(Ipv4Address ip)=>router.Interfaces.Values.Any(p=>p.Ipv4 is { } address && new Ipv4Network(address.Address,address.PrefixLength).Broadcast==ip);
        if ((received.DestinationMac.A & 1) != 0 || !Ipv4InterfaceAddress.IsUnicast(packet.SourceIp) || !Ipv4InterfaceAddress.IsUnicast(packet.DestinationIp)
            || OwnBroadcast(packet.SourceIp) || OwnBroadcast(packet.DestinationIp))
            throw new SimulationException(SimulationErrorCode.RouterIcmpErrorPathPending);
        if (router.Interfaces.Values.Any(p => p.Ipv4?.Address==packet.DestinationIp))
        {
            if (packet.Payload.Kind != IcmpKind.EchoRequest) return;
            var reply=new Ipv4Packet(packet.DestinationIp,packet.SourceIp,64,new IcmpPacket(IcmpKind.EchoReply,packet.Payload.Identifier,packet.Payload.Sequence));
            var returnRoute=LookupRouterRoute(ingress.Device,reply.DestinationIp) ?? throw new SimulationException(SimulationErrorCode.RouterIcmpErrorPathPending);
            QueueRouterIpv4(Endpoint.Of(ingress.Device,returnRoute.EgressInterface),returnRoute.NextHop ?? reply.DestinationIp,new(reply,null,null),AddTime(_currentTimeNs,DeviceProcessingDelayNs));
            return;
        }
        if (packet.Ttl<=1)
        {
            RecordRouterFailure(ingress,packet,DropReason.TtlExpiredInTransit);
            return;
        }
        var forwarded=packet with { Ttl=checked((byte)(packet.Ttl-1)) };
        var route=LookupRouterRoute(ingress.Device,packet.DestinationIp);
        if (route is null)
        {
            RecordRouterFailure(ingress,forwarded,DropReason.NoRouteToHost);
            return;
        }
        QueueRouterIpv4(Endpoint.Of(ingress.Device,route.EgressInterface),route.NextHop ?? packet.DestinationIp,new(forwarded,ingress,received),AddTime(_currentTimeNs,DeviceProcessingDelayNs));
    }

    private void RecordRouterFailure(Endpoint ingress,Ipv4Packet original,DropReason reason)
    {
        _trace.Add(new PacketDropped(_currentTimeNs,ingress.Device,ingress.InterfaceName,null,original,reason));
        if (original.Payload is IcmpErrorMessage) return;
        var route=LookupRouterRoute(ingress.Device,original.SourceIp);
        if (route is null) return;
        var endpoint=Endpoint.Of(ingress.Device,route.EgressInterface);
        var summary=OriginalDatagramSummary.From(original);
        IcmpMessage error=reason switch {
            DropReason.NoRouteToHost=>new DestinationNetworkUnreachableMessage(summary),
            DropReason.TtlExpiredInTransit=>new TimeExceededMessage(summary),
            _=>throw new InvalidOperationException("Not an implemented ICMP trigger.")
        };
        var response=new Ipv4Packet(FindInterface(endpoint).Ipv4!.Address,original.SourceIp,64,error);
        QueueRouterIpv4(endpoint,route.NextHop ?? original.SourceIp,new(response,null,null),AddTime(_currentTimeNs,DeviceProcessingDelayNs));
    }

    private void EmitRouterIpv4(Endpoint endpoint,MacAddress destination,Ipv4Packet packet,ulong at)
    {
        var port=FindInterface(endpoint);
        Schedule(at,new TransmitFrameEvent(endpoint,new(port.Mac!.Value,destination,packet)));
    }

    private void QueueRouterIpv4(Endpoint endpoint,Ipv4Address next,RouterPendingPacket packet,ulong at)
    {
        var router=_devices[endpoint.Device]; var key=(endpoint.InterfaceName,next);
        if (router.RouterArpCache.TryGetValue(key,out var cache)) { EmitRouterIpv4(endpoint,cache.Mac,packet.Packet,at); return; }
        var retryAt=AddTime(at,ArpRequestTimeoutNs);
        _trace.Add(new ArpPacketQueued(_currentTimeNs,endpoint.Device,next,packet.Packet) { ObservationInterface=endpoint.InterfaceName });
        if (!router.RouterPending.TryGetValue(key,out var pending)) { pending=[]; router.RouterPending.Add(key,pending); }
        pending.Add(packet);
        if (router.RouterResolutions.ContainsKey(key)) return;
        var epoch=NextEpoch(); router.RouterResolutions.Add(key,new ArpResolution(1,epoch,endpoint.InterfaceName));
        ScheduleArpRequest(endpoint,FindInterface(endpoint),next,at);
        Schedule(retryAt,new RouterArpRetryEvent(endpoint,next,1,epoch));
    }

    private bool RouterResolutionActive(RouterArpRetryEvent timer) =>
        _devices[timer.Endpoint.Device].RouterResolutions.TryGetValue((timer.Endpoint.InterfaceName,timer.Target),out var state) && state.Epoch==timer.Epoch && state.Attempt==timer.Attempt;

    private void RetryRouterArp(RouterArpRetryEvent timer)
    {
        if (!RouterResolutionActive(timer)) return;
        if (timer.Attempt>=3)
        {
            var owner=_devices[timer.Endpoint.Device];var pendingKey=(timer.Endpoint.InterfaceName,timer.Target);
            if (owner.RouterPending.TryGetValue(pendingKey,out var packets))
            {
                var errors=packets.FindAll(p=>p.Packet.Payload is IcmpErrorMessage);
                packets.RemoveAll(p=>p.Packet.Payload is IcmpErrorMessage);
                foreach (var error in errors)
                    _trace.Add(new PacketDropped(_currentTimeNs,timer.Endpoint.Device,timer.Endpoint.InterfaceName,null,error.Packet,DropReason.ArpResolutionFailed));
                if (packets.Count>0) throw new SimulationException(SimulationErrorCode.RouterIcmpErrorPathPending);
                owner.RouterPending.Remove(pendingKey);
            }
            owner.RouterResolutions.Remove(pendingKey);
            return;
        }
        var at=AddTime(_currentTimeNs,ArpRequestTimeoutNs); var attempt=checked((byte)(timer.Attempt+1));
        var key=(timer.Endpoint.InterfaceName,timer.Target); var router=_devices[timer.Endpoint.Device];
        router.RouterResolutions[key]=router.RouterResolutions[key] with { Attempt=attempt };
        if (InterfaceIsOperational(timer.Endpoint)) ScheduleArpRequest(timer.Endpoint,FindInterface(timer.Endpoint),timer.Target,_currentTimeNs);
        Schedule(at,new RouterArpRetryEvent(timer.Endpoint,timer.Target,attempt,timer.Epoch));
    }
}
