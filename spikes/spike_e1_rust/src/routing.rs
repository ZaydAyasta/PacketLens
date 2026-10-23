//! Routing configuration only; the caller supplies current operational state.
use crate::ipv4::{ConfigurationError, Ipv4InterfaceConfig, Ipv4Network};
use crate::{InterfaceName, Ipv4Address};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RouteEntry {
    pub network: Ipv4Network,
    pub next_hop: Option<Ipv4Address>,
    pub egress_interface: InterfaceName,
    pub metric: u32,
    pub is_connected: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RouterConfiguration {
    interfaces: BTreeMap<InterfaceName, Ipv4InterfaceConfig>,
    routes: Vec<RouteEntry>,
}

impl RouterConfiguration {
    pub fn new(
        interfaces: Vec<(InterfaceName, Ipv4InterfaceConfig)>,
    ) -> Result<Self, ConfigurationError> {
        let count = interfaces.len();
        let interfaces: BTreeMap<_, _> = interfaces.into_iter().collect();
        if count < 2
            || interfaces.len() != count
            || interfaces.keys().any(|n| n.0.trim().is_empty())
        {
            return Err(ConfigurationError::InvalidRouterConfiguration);
        }
        let mut networks = BTreeSet::new();
        if interfaces
            .values()
            .any(|iface| !networks.insert(iface.network()))
        {
            return Err(ConfigurationError::DuplicateConnectedPrefix);
        }
        let routes = interfaces
            .iter()
            .map(|(name, config)| RouteEntry {
                network: config.network(),
                next_hop: None,
                egress_interface: name.clone(),
                metric: 0,
                is_connected: true,
            })
            .collect();
        Ok(Self { interfaces, routes })
    }
    pub fn routes(&self) -> &[RouteEntry] {
        &self.routes
    }
    pub fn configure_interface(
        &mut self,
        name: &InterfaceName,
        config: Ipv4InterfaceConfig,
    ) -> Result<(), ConfigurationError> {
        if !self.interfaces.contains_key(name) {
            return Err(ConfigurationError::UnknownInterface);
        }
        let mut interfaces = self.interfaces.clone();
        interfaces.insert(name.clone(), config);
        let mut candidate = Self::new(interfaces.into_iter().collect())?;
        for route in self.routes.iter().filter(|r| !r.is_connected) {
            candidate.add_static_route(
                route.network,
                route.next_hop,
                route.egress_interface.clone(),
                route.metric,
            )?;
        }
        *self = candidate;
        Ok(())
    }
    pub fn add_static_route(
        &mut self,
        network: Ipv4Network,
        next_hop: Option<Ipv4Address>,
        egress_interface: InterfaceName,
        metric: u32,
    ) -> Result<(), ConfigurationError> {
        let iface = self
            .interfaces
            .get(&egress_interface)
            .ok_or(ConfigurationError::UnknownInterface)?;
        if let Some(existing) = self.routes.iter().find(|route| route.network == network) {
            return Err(if existing.is_connected {
                ConfigurationError::PrefixOverlapsConnectedNetwork
            } else {
                ConfigurationError::AmbiguousRouteConfig
            });
        }
        if let Some(ip) = next_hop {
            if !crate::ipv4::is_unicast(ip) || self.interfaces.values().any(|i| i.address() == ip) {
                return Err(ConfigurationError::InvalidNextHop);
            }
            if !iface.network().contains(ip) {
                return Err(ConfigurationError::NextHopNotDirectlyConnected);
            }
            if !iface.is_usable_on_link(ip) {
                return Err(ConfigurationError::InvalidNextHop);
            }
        }
        self.routes.push(RouteEntry {
            network,
            next_hop,
            egress_interface,
            metric,
            is_connected: false,
        });
        Ok(())
    }
    pub fn lookup_route(
        &self,
        target: Ipv4Address,
        is_operational: impl Fn(&InterfaceName) -> bool,
    ) -> Option<&RouteEntry> {
        self.routes
            .iter()
            .filter(|r| is_operational(&r.egress_interface) && r.network.contains(target))
            .max_by_key(|r| r.network.prefix())
    }
}
