//! E2 router protocol state, separate from configuration and presentation.
use crate::*;

#[cfg(test)]
mod admission_tests;

#[derive(Clone, Debug, Default)]
pub(super) struct RouterArpState {
    cache: BTreeMap<Ipv4Address, ArpEntry>,
    pending: BTreeMap<Ipv4Address, VecDeque<RouterPending>>,
    resolutions: BTreeMap<Ipv4Address, ActiveResolution>,
}

#[derive(Clone, Debug)]
struct RouterPending {
    packet: Ipv4Packet,
    // Original reception retained for future ICMP suppression; not inferred causality.
    _received: Option<(EndpointRef, EthernetFrame)>,
}

fn is_icmp_error(packet: &Ipv4Packet) -> bool {
    match &packet.payload {
        Ipv4Payload::IcmpError(_) => true,
        Ipv4Payload::Icmp(message) => matches!(
            message.kind,
            IcmpKind::TimeExceeded | IcmpKind::DestinationHostUnreachable
        ),
    }
}

impl Engine {
    fn emit_router_scope(
        &mut self,
        endpoint: &EndpointRef,
        event: ObservableEvent,
    ) -> Result<(), EngineError> {
        self.traces
            .emit_scoped(self.now, event, Some(endpoint.interface.clone()))
    }

    pub(super) fn router_received_frame(
        &mut self,
        endpoint: EndpointRef,
        frame: EthernetFrame,
    ) -> Result<(), EngineError> {
        let port = self.interface(&endpoint)?;
        let mac = port
            .mac
            .ok_or_else(|| EngineError::InvalidHostConfiguration(endpoint.device.clone()))?;
        let own_ip = port
            .ipv4
            .ok_or_else(|| EngineError::InvalidHostConfiguration(endpoint.device.clone()))?
            .0;
        let group_ipv4 =
            frame.dst_mac.0[0] & 1 != 0 && matches!(frame.payload, EthernetPayload::Ipv4(_));
        if frame.dst_mac != mac && frame.dst_mac != MacAddress::BROADCAST && !group_ipv4 {
            return self.drop_at(endpoint, frame, DropReason::DestinationMacMismatch);
        }
        let arp = match frame.payload.clone() {
            EthernetPayload::Arp(arp) => arp,
            EthernetPayload::Ipv4(packet) => {
                return self.router_received_ipv4(endpoint, frame, packet);
            }
        };
        if arp.target_ip != own_ip
            || (arp.operation == ArpOperation::Reply && arp.target_mac != mac)
        {
            return self.drop_at(endpoint, frame, DropReason::ArpTargetIpMismatch);
        }
        self.learn_router_arp(&endpoint, arp.sender_ip, arp.sender_mac)?;
        if arp.operation == ArpOperation::Request {
            let reply = EthernetFrame {
                src_mac: mac,
                dst_mac: arp.sender_mac,
                payload: EthernetPayload::Arp(ArpPacket {
                    operation: ArpOperation::Reply,
                    sender_mac: mac,
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
            )?;
        } else {
            let (pending, active) = {
                let state = self.router_arp_mut(&endpoint)?;
                (
                    state.pending.remove(&arp.sender_ip).unwrap_or_default(),
                    state.resolutions.remove(&arp.sender_ip).is_some(),
                )
            };
            if active {
                self.remove_active_scheduled_event();
            }
            let at = self
                .now
                .0
                .checked_add(DEVICE_PROCESSING_DELAY_NS)
                .ok_or(EngineError::TimeOverflow)?;
            for pending in pending {
                self.router_send_resolved(&endpoint, arp.sender_mac, pending.packet, at)?;
            }
        }
        Ok(())
    }

    fn learn_router_arp(
        &mut self,
        endpoint: &EndpointRef,
        ip: Ipv4Address,
        mac: MacAddress,
    ) -> Result<(), EngineError> {
        let expires = self
            .now
            .0
            .checked_add(ARP_CACHE_TTL_NS)
            .ok_or(EngineError::TimeOverflow)?;
        let epoch = self.next_epoch_value()?;
        let Some(Device::Router(r)) = self.devices.get_mut(&endpoint.device) else {
            return Err(EngineError::InvalidHostConfiguration(
                endpoint.device.clone(),
            ));
        };
        let state = r
            .arp
            .get_mut(&endpoint.interface)
            .ok_or_else(|| EngineError::UnknownInterface(endpoint.clone()))?;
        state.cache.insert(ip, ArpEntry { mac, epoch });
        self.emit_router_scope(
            endpoint,
            ObservableEvent::TableUpdated {
                at_device: endpoint.device.clone(),
                update: TableUpdateDetail::ArpLearned {
                    ip,
                    mac,
                    expires_at: VirtualTime(expires),
                },
            },
        )?;
        self.schedule(
            expires,
            InternalEvent::RouterArpExpireTimer {
                endpoint: endpoint.clone(),
                ip,
                entry_epoch: epoch,
            },
        )
    }

    pub(super) fn expire_router_arp(
        &mut self,
        endpoint: EndpointRef,
        ip: Ipv4Address,
        epoch: u64,
    ) -> Result<(), EngineError> {
        let expired = if let Some(Device::Router(r)) = self.devices.get_mut(&endpoint.device) {
            if let Some(state) = r.arp.get_mut(&endpoint.interface) {
                if state.cache.get(&ip).is_some_and(|e| e.epoch == epoch) {
                    state.cache.remove(&ip);
                    true
                } else {
                    false
                }
            } else {
                false
            }
        } else {
            false
        };
        if expired {
            self.emit_router_scope(
                &endpoint,
                ObservableEvent::TableUpdated {
                    at_device: endpoint.device.clone(),
                    update: TableUpdateDetail::ArpEntryExpired { ip },
                },
            )?;
        }
        Ok(())
    }

    pub fn router_arp_entry(
        &self,
        endpoint: &EndpointRef,
        ip: Ipv4Address,
    ) -> Result<Option<MacAddress>, EngineError> {
        self.interface(endpoint)?;
        let Some(Device::Router(r)) = self.devices.get(&endpoint.device) else {
            return Err(EngineError::InvalidHostConfiguration(
                endpoint.device.clone(),
            ));
        };
        Ok(r.arp
            .get(&endpoint.interface)
            .and_then(|s| s.cache.get(&ip))
            .map(|entry| entry.mac))
    }

    fn router_arp_mut(
        &mut self,
        endpoint: &EndpointRef,
    ) -> Result<&mut RouterArpState, EngineError> {
        let Some(Device::Router(r)) = self.devices.get_mut(&endpoint.device) else {
            return Err(EngineError::InvalidHostConfiguration(
                endpoint.device.clone(),
            ));
        };
        r.arp
            .get_mut(&endpoint.interface)
            .ok_or_else(|| EngineError::UnknownInterface(endpoint.clone()))
    }

    pub(super) fn router_resolution_active(
        &self,
        endpoint: &EndpointRef,
        target: Ipv4Address,
        attempt: u8,
        epoch: u64,
    ) -> bool {
        matches!(self.devices.get(&endpoint.device),Some(Device::Router(r)) if r.arp.get(&endpoint.interface).and_then(|s|s.resolutions.get(&target)).is_some_and(|a| a.epoch==epoch && a.attempt==attempt))
    }

    fn router_received_ipv4(
        &mut self,
        ingress: EndpointRef,
        original: EthernetFrame,
        mut packet: Ipv4Packet,
    ) -> Result<(), EngineError> {
        let own_broadcast = |ip| {
            self.devices.get(&ingress.device).is_some_and(|d| {
                d.interfaces().values().any(|p| {
                    p.ipv4.is_some_and(|(address, prefix)| {
                        crate::ipv4::Ipv4Network::new(address, prefix)
                            .is_ok_and(|n| n.broadcast() == Some(ip))
                    })
                })
            })
        };
        if original.dst_mac.0[0] & 1 != 0
            || (224..=239).contains(&packet.dst_ip.0[0])
            || packet.dst_ip == Ipv4Address([255; 4])
            || own_broadcast(packet.dst_ip)
        {
            return self.emit(ObservableEvent::PacketDropped {
                at_device: ingress.device,
                at_interface: Some(ingress.interface),
                pdu: DroppedPdu::Ipv4(packet),
                reason: DropReason::UnsupportedIpDestination,
            });
        }
        if matches!(packet.src_ip.0[0], 0 | 127 | 224..=239)
            || packet.src_ip == Ipv4Address([255; 4])
            || own_broadcast(packet.src_ip)
        {
            return self.emit(ObservableEvent::PacketDropped {
                at_device: ingress.device,
                at_interface: Some(ingress.interface),
                pdu: DroppedPdu::Ipv4(packet),
                reason: DropReason::InvalidIpv4Source,
            });
        }
        // A locally delivered Request is not a transit packet. The Reply is newly originated.
        let local = self.devices.get(&ingress.device).is_some_and(|d| {
            d.interfaces()
                .values()
                .any(|p| p.ipv4.is_some_and(|(ip, _)| ip == packet.dst_ip))
        });
        if local {
            let Ipv4Payload::Icmp(icmp) = &packet.payload else {
                return Ok(());
            };
            if icmp.kind != IcmpKind::EchoRequest {
                return Ok(());
            }
            let reply = Ipv4Packet {
                src_ip: packet.dst_ip,
                dst_ip: packet.src_ip,
                ttl: 64,
                payload: Ipv4Payload::Icmp(IcmpPacket {
                    kind: IcmpKind::EchoReply,
                    identifier: icmp.identifier,
                    sequence: icmp.sequence,
                }),
            };
            let route = self
                .lookup_router_route(&ingress.device, reply.dst_ip)?
                .ok_or(EngineError::RouterIcmpErrorPathPending)?;
            let endpoint = EndpointRef {
                device: ingress.device,
                interface: route.egress_interface,
            };
            let next = route.next_hop.unwrap_or(reply.dst_ip);
            let at = self
                .now
                .0
                .checked_add(DEVICE_PROCESSING_DELAY_NS)
                .ok_or(EngineError::TimeOverflow)?;
            return self.router_send_ipv4(
                endpoint,
                next,
                RouterPending {
                    packet: reply,
                    _received: None,
                },
                at,
            );
        }
        if packet.ttl <= 1 {
            return self.router_icmp_failure(ingress, packet, DropReason::TtlExpiredInTransit);
        }
        packet.ttl -= 1;
        let Some(route) = self.lookup_router_route(&ingress.device, packet.dst_ip)? else {
            return self.router_icmp_failure(ingress, packet, DropReason::NoRouteToHost);
        };
        let endpoint = EndpointRef {
            device: ingress.device.clone(),
            interface: route.egress_interface,
        };
        let next = route.next_hop.unwrap_or(packet.dst_ip);
        let at = self
            .now
            .0
            .checked_add(DEVICE_PROCESSING_DELAY_NS)
            .ok_or(EngineError::TimeOverflow)?;
        self.router_send_ipv4(
            endpoint,
            next,
            RouterPending {
                packet,
                _received: Some((ingress, original)),
            },
            at,
        )
    }

    fn router_icmp_failure(
        &mut self,
        ingress: EndpointRef,
        packet: Ipv4Packet,
        reason: DropReason,
    ) -> Result<(), EngineError> {
        self.emit(ObservableEvent::PacketDropped {
            at_device: ingress.device.clone(),
            at_interface: Some(ingress.interface.clone()),
            pdu: DroppedPdu::Ipv4(packet.clone()),
            reason,
        })?;
        // The new error PDU may be forwarded, but never answered with another error.
        if is_icmp_error(&packet) {
            return Ok(());
        }
        let Some(route) = self.lookup_router_route(&ingress.device, packet.src_ip)? else {
            return Ok(());
        };
        let endpoint = EndpointRef {
            device: ingress.device,
            interface: route.egress_interface,
        };
        let source = self
            .interface(&endpoint)?
            .ipv4
            .ok_or_else(|| EngineError::InvalidHostConfiguration(endpoint.device.clone()))?
            .0;
        let summary = OriginalDatagramSummary::from_ipv4(&packet);
        let error = match reason {
            DropReason::TtlExpiredInTransit => IcmpErrorPacket::TimeExceeded { original: summary },
            DropReason::NoRouteToHost => {
                IcmpErrorPacket::DestinationNetworkUnreachable { original: summary }
            }
            _ => unreachable!("Only implemented router ICMP triggers enter here"),
        };
        let response = Ipv4Packet {
            src_ip: source,
            dst_ip: packet.src_ip,
            ttl: 64,
            payload: Ipv4Payload::IcmpError(error),
        };
        let at = self
            .now
            .0
            .checked_add(DEVICE_PROCESSING_DELAY_NS)
            .ok_or(EngineError::TimeOverflow)?;
        self.router_send_ipv4(
            endpoint,
            route.next_hop.unwrap_or(packet.src_ip),
            RouterPending {
                packet: response,
                _received: None,
            },
            at,
        )
    }

    fn router_send_resolved(
        &mut self,
        endpoint: &EndpointRef,
        destination: MacAddress,
        packet: Ipv4Packet,
        at: u64,
    ) -> Result<(), EngineError> {
        let source = self
            .interface(endpoint)?
            .mac
            .ok_or_else(|| EngineError::InvalidHostConfiguration(endpoint.device.clone()))?;
        self.schedule(
            at,
            InternalEvent::TransmitFrame {
                from: endpoint.clone(),
                frame: EthernetFrame {
                    src_mac: source,
                    dst_mac: destination,
                    payload: EthernetPayload::Ipv4(packet),
                },
            },
        )
    }

    fn router_send_ipv4(
        &mut self,
        endpoint: EndpointRef,
        next: Ipv4Address,
        pending: RouterPending,
        at: u64,
    ) -> Result<(), EngineError> {
        if let Some(mac) = self.router_arp_entry(&endpoint, next)? {
            return self.router_send_resolved(&endpoint, mac, pending.packet, at);
        }
        let retry = at
            .checked_add(ARP_REQUEST_TIMEOUT_NS)
            .ok_or(EngineError::TimeOverflow)?;
        self.emit_router_scope(
            &endpoint,
            ObservableEvent::ArpPacketQueued {
                at_device: endpoint.device.clone(),
                target_ip: next,
                packet: pending.packet.clone(),
            },
        )?;
        let state = self.router_arp_mut(&endpoint)?;
        state.pending.entry(next).or_default().push_back(pending);
        if state.resolutions.contains_key(&next) {
            return Ok(());
        }
        let epoch = self.next_epoch_value()?;
        self.router_arp_mut(&endpoint)?.resolutions.insert(
            next,
            ActiveResolution {
                attempt: 1,
                epoch,
                iface: endpoint.interface.clone(),
            },
        );
        self.schedule_arp_request(
            endpoint.device.clone(),
            endpoint.interface.clone(),
            next,
            at,
        )?;
        self.schedule(
            retry,
            InternalEvent::RouterArpRetryTimer {
                endpoint,
                target_ip: next,
                attempt: 1,
                resolution_epoch: epoch,
            },
        )
    }

    pub(super) fn retry_router_arp(
        &mut self,
        endpoint: EndpointRef,
        target: Ipv4Address,
        attempt: u8,
        epoch: u64,
    ) -> Result<(), EngineError> {
        if !self.router_resolution_active(&endpoint, target, attempt, epoch) {
            return Ok(());
        }
        if attempt < ARP_MAX_ATTEMPTS {
            let next = attempt + 1;
            let at = self
                .now
                .0
                .checked_add(ARP_REQUEST_TIMEOUT_NS)
                .ok_or(EngineError::TimeOverflow)?;
            if let Some(active) = self.router_arp_mut(&endpoint)?.resolutions.get_mut(&target) {
                active.attempt = next;
            }
            if self.interface_is_operational(&endpoint)? {
                self.schedule_arp_request(
                    endpoint.device.clone(),
                    endpoint.interface.clone(),
                    target,
                    self.now.0,
                )?;
            }
            self.schedule(
                at,
                InternalEvent::RouterArpRetryTimer {
                    endpoint,
                    target_ip: target,
                    attempt: next,
                    resolution_epoch: epoch,
                },
            )
        } else {
            // Discard errors, including legacy tags, without a secondary error.
            // Ordinary originals and Host Unreachable remain pending for 05b.
            let pending = self
                .router_arp_mut(&endpoint)?
                .pending
                .remove(&target)
                .unwrap_or_default();
            let (errors, originals): (VecDeque<_>, VecDeque<_>) =
                pending.into_iter().partition(|p| is_icmp_error(&p.packet));
            let has_originals = !originals.is_empty();
            if has_originals {
                self.router_arp_mut(&endpoint)?
                    .pending
                    .insert(target, originals);
            } else {
                self.router_arp_mut(&endpoint)?.resolutions.remove(&target);
            }
            for error in errors {
                self.emit(ObservableEvent::PacketDropped {
                    at_device: endpoint.device.clone(),
                    at_interface: Some(endpoint.interface.clone()),
                    pdu: DroppedPdu::Ipv4(error.packet),
                    reason: DropReason::ArpResolutionFailed,
                })?;
            }
            if has_originals {
                Err(EngineError::RouterIcmpErrorPathPending)
            } else {
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ipv4::{HostIpv4Config, Ipv4InterfaceConfig};
    use crate::router_engine::RouterPortConfig;

    fn fixture() -> Engine {
        let mut e = Engine::with_default_trace_capacity();
        for (name, ip, mac) in [
            (
                "h1",
                Ipv4Address([192, 168, 1, 10]),
                MacAddress([2, 0, 0, 0, 1, 1]),
            ),
            (
                "h2",
                Ipv4Address([10, 0, 0, 20]),
                MacAddress([2, 0, 0, 0, 2, 2]),
            ),
        ] {
            e.create_host(name.into(), vec!["p".into()]).unwrap();
            e.configure_host_ipv4(
                EndpointRef::new(name, "p"),
                mac,
                HostIpv4Config::new(Ipv4InterfaceConfig::new(ip, 24).unwrap(), None).unwrap(),
                true,
            )
            .unwrap();
        }
        e.create_router(
            "r".into(),
            vec![
                RouterPortConfig::new(
                    "a".into(),
                    MacAddress([2, 0, 0, 0, 1, 254]),
                    Ipv4InterfaceConfig::new(Ipv4Address([192, 168, 1, 1]), 24).unwrap(),
                    true,
                ),
                RouterPortConfig::new(
                    "b".into(),
                    MacAddress([2, 0, 0, 0, 2, 254]),
                    Ipv4InterfaceConfig::new(Ipv4Address([10, 0, 0, 1]), 24).unwrap(),
                    true,
                ),
            ],
        )
        .unwrap();
        e.connect_link(
            "left".into(),
            EndpointRef::new("h1", "p"),
            EndpointRef::new("r", "a"),
        )
        .unwrap();
        e.connect_link(
            "right".into(),
            EndpointRef::new("r", "b"),
            EndpointRef::new("h2", "p"),
        )
        .unwrap();
        e
    }
    fn inject(e: &mut Engine, ttl: u8, source: Ipv4Address, target: Ipv4Address, broadcast: bool) {
        let dst = EndpointRef::new("r", "a");
        let instance = e.interface(&dst).unwrap().attached_link.unwrap();
        e.schedule(
            0,
            InternalEvent::FrameArrived {
                link_instance: instance,
                dst,
                frame: EthernetFrame {
                    src_mac: MacAddress([2, 0, 0, 0, 1, 1]),
                    dst_mac: if broadcast {
                        MacAddress::BROADCAST
                    } else {
                        MacAddress([2, 0, 0, 0, 1, 254])
                    },
                    payload: EthernetPayload::Ipv4(Ipv4Packet {
                        src_ip: source,
                        dst_ip: target,
                        ttl,
                        payload: Ipv4Payload::Icmp(IcmpPacket {
                            kind: IcmpKind::EchoRequest,
                            identifier: 42,
                            sequence: 1,
                        }),
                    }),
                },
            },
        )
        .unwrap();
    }
    #[test]
    fn local_echo_before_transit_ttl_zero_or_one_uses_requested_ip() {
        for ttl in [0, 1] {
            for target in [Ipv4Address([192, 168, 1, 1]), Ipv4Address([10, 0, 0, 1])] {
                let mut e = fixture();
                inject(&mut e, ttl, Ipv4Address([192, 168, 1, 10]), target, false);
                e.run_until_quiescent(100).unwrap();
                assert_eq!(e.get_trace_snapshot().records.iter().filter(|r|matches!(&r.event,ObservableEvent::FrameTransmitted {from,frame:EthernetFrame {payload:EthernetPayload::Ipv4(p),..},..} if from.device.0=="r" && p.src_ip==target && p.ttl==64)).count(),1);
                assert!(!e.get_trace_snapshot().records.iter().any(|r| matches!(
                    r.event,
                    ObservableEvent::PingCompleted { .. } | ObservableEvent::PingFailed { .. }
                )));
            }
        }
    }
    #[test]
    fn inadmissible_ipv4_has_normative_drop_before_forwarding_or_arp() {
        for (ttl, source, target, broadcast, expected) in [
            (
                64,
                Ipv4Address([192, 168, 1, 10]),
                Ipv4Address([192, 168, 1, 1]),
                true,
                DropReason::UnsupportedIpDestination,
            ),
            (
                64,
                Ipv4Address([0, 0, 0, 0]),
                Ipv4Address([192, 168, 1, 1]),
                false,
                DropReason::InvalidIpv4Source,
            ),
            (
                64,
                Ipv4Address([192, 168, 1, 255]),
                Ipv4Address([192, 168, 1, 1]),
                false,
                DropReason::InvalidIpv4Source,
            ),
            (
                64,
                Ipv4Address([192, 168, 1, 10]),
                Ipv4Address([10, 0, 0, 255]),
                false,
                DropReason::UnsupportedIpDestination,
            ),
            (
                64,
                Ipv4Address([192, 168, 1, 10]),
                Ipv4Address([224, 0, 0, 1]),
                false,
                DropReason::UnsupportedIpDestination,
            ),
        ] {
            let mut e = fixture();
            inject(&mut e, ttl, source, target, broadcast);
            e.run_until_quiescent(1000).unwrap();
            let trace = e.get_trace_snapshot();
            assert_eq!(trace.records.len(), 2);
            assert!(matches!(
                &trace.records[0].event,
                ObservableEvent::FrameDelivered { .. }
            ));
            assert!(
                matches!(&trace.records[1].event,ObservableEvent::PacketDropped {reason,pdu:DroppedPdu::Ipv4(p),..} if *reason==expected && p.src_ip==source && p.dst_ip==target && p.ttl==ttl)
            );
            assert_eq!(e.now().0, 0);
        }
    }

    #[test]
    fn renewed_router_arp_ignores_old_expiry_and_expires_at_new_epoch() {
        let mut e = fixture();
        let inject_arp = |e: &mut Engine, at: u64| {
            let dst = EndpointRef::new("r", "a");
            let link_instance = e.interface(&dst).unwrap().attached_link.unwrap();
            let mac = MacAddress([2, 0, 0, 0, 1, 1]);
            e.schedule(
                at,
                InternalEvent::FrameArrived {
                    link_instance,
                    dst,
                    frame: EthernetFrame {
                        src_mac: mac,
                        dst_mac: MacAddress::BROADCAST,
                        payload: EthernetPayload::Arp(ArpPacket {
                            operation: ArpOperation::Request,
                            sender_mac: mac,
                            sender_ip: Ipv4Address([192, 168, 1, 10]),
                            target_mac: MacAddress([0; 6]),
                            target_ip: Ipv4Address([192, 168, 1, 1]),
                        }),
                    },
                },
            )
            .unwrap();
        };
        inject_arp(&mut e, 0);
        e.run_until_quiescent(100).unwrap();
        e.advance_until(VirtualTime(10000), 100).unwrap();
        inject_arp(&mut e, 10000);
        e.run_until_quiescent(100).unwrap();
        let before = e.get_trace_snapshot();
        e.advance_until(VirtualTime(ARP_CACHE_TTL_NS), 100).unwrap();
        assert_eq!(e.get_trace_snapshot(), before);
        assert_eq!(
            e.router_arp_entry(&EndpointRef::new("r", "a"), Ipv4Address([192, 168, 1, 10]))
                .unwrap(),
            Some(MacAddress([2, 0, 0, 0, 1, 1]))
        );
        e.advance_until(VirtualTime(ARP_CACHE_TTL_NS + 10000), 100)
            .unwrap();
        assert_eq!(
            e.router_arp_entry(&EndpointRef::new("r", "a"), Ipv4Address([192, 168, 1, 10]))
                .unwrap(),
            None
        );
        assert!(e.get_trace_snapshot().records.iter().any(|r|r.at.0==ARP_CACHE_TTL_NS+10000 && r.observation_interface==Some("a".into()) && matches!(&r.event,ObservableEvent::TableUpdated {at_device,update:TableUpdateDetail::ArpEntryExpired { .. }} if at_device.0=="r")));
    }
}
