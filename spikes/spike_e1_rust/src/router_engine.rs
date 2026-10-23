//! Router configuration attached to real DES interfaces. No packet forwarding.
use crate::ipv4::{ConfigurationError, Ipv4InterfaceConfig, Ipv4Network};
use crate::routing::{RouteEntry, RouterConfiguration};
use crate::*;

#[derive(Clone, Debug)]
pub struct RouterPortConfig {
    pub name: InterfaceName,
    pub mac: MacAddress,
    pub ipv4: Ipv4InterfaceConfig,
    pub admin_up: bool,
    pub mode: ipv4::Ipv4LinkMode,
}

impl RouterPortConfig {
    pub fn new(
        name: InterfaceName,
        mac: MacAddress,
        ipv4: Ipv4InterfaceConfig,
        admin_up: bool,
    ) -> Self {
        Self {
            name,
            mac,
            ipv4,
            admin_up,
            mode: ipv4::Ipv4LinkMode::MultiAccess,
        }
    }
}

#[derive(Clone, Debug)]
pub(super) struct Router {
    pub(super) interfaces: BTreeMap<InterfaceName, Interface>,
    configuration: RouterConfiguration,
}

impl Engine {
    pub fn set_interface_ipv4_mode(
        &mut self,
        endpoint: EndpointRef,
        mode: ipv4::Ipv4LinkMode,
    ) -> Result<(), EngineError> {
        self.ensure_ipv4_configuration_idle()?;
        self.interface_mut(&endpoint)?.ipv4_mode = mode;
        Ok(())
    }

    pub fn validate_ipv4_topology(&self) -> Result<(), EngineError> {
        for (device, node) in &self.devices {
            for (name, port) in node.interfaces() {
                if port.ipv4.is_some_and(|(_, prefix)| prefix == 31)
                    && !self.is_valid_point_to_point(&EndpointRef {
                        device: device.clone(),
                        interface: name.clone(),
                    })
                {
                    return Err(EngineError::Ipv4Configuration(
                        ConfigurationError::InvalidPointToPointTopology,
                    ));
                }
            }
        }
        Ok(())
    }

    fn is_valid_point_to_point(&self, endpoint: &EndpointRef) -> bool {
        let Some(Device::Host(_) | Device::Router(_)) = self.devices.get(&endpoint.device) else {
            return false;
        };
        let Ok(port) = self.interface(endpoint) else {
            return false;
        };
        if port.ipv4_mode != ipv4::Ipv4LinkMode::PointToPoint {
            return false;
        }
        let Some(instance) = port.attached_link else {
            return false;
        };
        let Some(link) = self
            .links_by_instance
            .get(&instance)
            .and_then(|n| self.links.get(n))
        else {
            return false;
        };
        if link.instance != instance {
            return false;
        }
        let other = if &link.a == endpoint {
            &link.b
        } else if &link.b == endpoint {
            &link.a
        } else {
            return false;
        };
        let Some(Device::Host(_) | Device::Router(_)) = self.devices.get(&other.device) else {
            return false;
        };
        let Ok(peer) = self.interface(other) else {
            return false;
        };
        if peer.attached_link != Some(instance)
            || peer.ipv4_mode != ipv4::Ipv4LinkMode::PointToPoint
        {
            return false;
        }
        match (port.ipv4, peer.ipv4) {
            (Some((a, 31)), Some((b, 31))) => {
                a != b
                    && ipv4::is_unicast(a)
                    && ipv4::is_unicast(b)
                    && Ipv4Network::new(a, 31).is_ok_and(|n| n.contains(b))
            }
            _ => false,
        }
    }

    pub fn create_router(
        &mut self,
        device: DeviceRef,
        ports: Vec<RouterPortConfig>,
    ) -> Result<(), EngineError> {
        self.ensure_ipv4_configuration_idle()?;
        if self.devices.contains_key(&device) {
            return Err(EngineError::DuplicateId(device.0));
        }
        unique_interfaces(ports.iter().map(|p| p.name.clone()).collect())?;
        let configuration =
            RouterConfiguration::new(ports.iter().map(|p| (p.name.clone(), p.ipv4)).collect())
                .map_err(EngineError::Ipv4Configuration)?;
        let interfaces = ports
            .into_iter()
            .map(|p| {
                (
                    p.name,
                    Interface {
                        ipv4_mode: p.mode,
                        mac: Some(p.mac),
                        ipv4: Some((p.ipv4.address(), p.ipv4.network().prefix().value())),
                        admin_up: p.admin_up,
                        attached_link: None,
                    },
                )
            })
            .collect();
        self.devices.insert(
            device,
            Device::Router(Router {
                interfaces,
                configuration,
            }),
        );
        Ok(())
    }

    fn router(&self, device: &DeviceRef) -> Result<&Router, EngineError> {
        match self.devices.get(device) {
            Some(Device::Router(r)) => Ok(r),
            Some(_) => Err(EngineError::Ipv4Configuration(
                ConfigurationError::InvalidRouterConfiguration,
            )),
            None => Err(EngineError::UnknownDevice(device.clone())),
        }
    }

    pub fn router_routes(&self, device: &DeviceRef) -> Result<Vec<RouteEntry>, EngineError> {
        Ok(self.router(device)?.configuration.routes().to_vec())
    }

    pub fn interface_ipv4(
        &self,
        endpoint: &EndpointRef,
    ) -> Result<Option<(Ipv4Address, u8)>, EngineError> {
        Ok(self.interface(endpoint)?.ipv4)
    }

    pub fn interface_is_operational(&self, endpoint: &EndpointRef) -> Result<bool, EngineError> {
        let port = self.interface(endpoint)?;
        Ok(port.admin_up
            && port.attached_link.is_some_and(|instance| {
                self.links_by_instance
                    .get(&instance)
                    .and_then(|name| self.links.get(name))
                    .is_some_and(|link| {
                        link.instance == instance && (&link.a == endpoint || &link.b == endpoint)
                    })
            }))
    }

    pub fn lookup_router_route(
        &self,
        device: &DeviceRef,
        destination: Ipv4Address,
    ) -> Result<Option<RouteEntry>, EngineError> {
        let router = self.router(device)?;
        Ok(router
            .configuration
            .lookup_route(destination, |name| {
                self.interface_is_operational(&EndpointRef {
                    device: device.clone(),
                    interface: name.clone(),
                })
                .unwrap_or(false)
            })
            .cloned())
    }

    pub fn add_router_static_route(
        &mut self,
        device: &DeviceRef,
        network: Ipv4Network,
        next_hop: Option<Ipv4Address>,
        egress: InterfaceName,
        metric: u32,
    ) -> Result<(), EngineError> {
        self.ensure_ipv4_configuration_idle()?;
        self.router(device)?;
        if let Some(Device::Router(router)) = self.devices.get_mut(device) {
            router
                .configuration
                .add_static_route(network, next_hop, egress, metric)
                .map_err(EngineError::Ipv4Configuration)?;
        }
        Ok(())
    }

    pub(super) fn configure_router_port(
        &mut self,
        endpoint: EndpointRef,
        mac: MacAddress,
        address: Option<(Ipv4Address, u8)>,
        admin_up: bool,
    ) -> Result<(), EngineError> {
        self.ensure_ipv4_configuration_idle()?;
        self.interface(&endpoint)?;
        let (ip, prefix) = address.ok_or(EngineError::Ipv4Configuration(
            ConfigurationError::InvalidInterfaceAddress,
        ))?;
        let config =
            Ipv4InterfaceConfig::new(ip, prefix).map_err(EngineError::Ipv4Configuration)?;
        if let Some(Device::Router(router)) = self.devices.get_mut(&endpoint.device) {
            router
                .configuration
                .configure_interface(&endpoint.interface, config)
                .map_err(EngineError::Ipv4Configuration)?;
            let port = router
                .interfaces
                .get_mut(&endpoint.interface)
                .ok_or_else(|| EngineError::UnknownInterface(endpoint.clone()))?;
            port.mac = Some(mac);
            port.ipv4 = address;
            port.admin_up = admin_up;
        }
        Ok(())
    }
}
