//! E2.1 configuration values. No packet processing or presentation dependency.
use crate::Ipv4Address;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Ipv4LinkMode {
    #[default]
    MultiAccess,
    PointToPoint,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Ipv4PrefixLength(u8);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfigurationError {
    InvalidPrefixLength(u8),
    InvalidInterfaceAddress,
    InvalidGateway,
    InvalidRouterConfiguration,
    UnknownInterface,
    DuplicateConnectedPrefix,
    AmbiguousRouteConfig,
    PrefixOverlapsConnectedNetwork,
    NextHopNotDirectlyConnected,
    InvalidNextHop,
    InvalidPointToPointTopology,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Ipv4Network {
    address: Ipv4Address,
    prefix: Ipv4PrefixLength,
}

impl Ipv4Network {
    pub fn new(address: Ipv4Address, length: u8) -> Result<Self, ConfigurationError> {
        let prefix = Ipv4PrefixLength::new(length)?;
        let address =
            Ipv4Address((u32::from_be_bytes(address.0) & prefix.netmask_u32()).to_be_bytes());
        Ok(Self { address, prefix })
    }
    pub fn address(self) -> Ipv4Address {
        self.address
    }
    pub fn prefix(self) -> Ipv4PrefixLength {
        self.prefix
    }
    pub fn contains(self, ip: Ipv4Address) -> bool {
        u32::from_be_bytes(ip.0) & self.prefix.netmask_u32() == u32::from_be_bytes(self.address.0)
    }
    pub fn broadcast(self) -> Option<Ipv4Address> {
        (self.prefix.value() < 31).then(|| {
            Ipv4Address(
                (u32::from_be_bytes(self.address.0) | !self.prefix.netmask_u32()).to_be_bytes(),
            )
        })
    }
}

pub(crate) fn is_unicast(ip: Ipv4Address) -> bool {
    ip.0[0] != 0 && ip.0[0] != 127 && ip.0[0] < 224
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ipv4InterfaceConfig {
    address: Ipv4Address,
    network: Ipv4Network,
}

impl Ipv4InterfaceConfig {
    pub fn new(address: Ipv4Address, length: u8) -> Result<Self, ConfigurationError> {
        let value = Self {
            address,
            network: Ipv4Network::new(address, length)?,
        };
        if !value.is_usable_on_link(address) {
            return Err(ConfigurationError::InvalidInterfaceAddress);
        }
        Ok(value)
    }
    pub fn address(self) -> Ipv4Address {
        self.address
    }
    pub fn network(self) -> Ipv4Network {
        self.network
    }
    pub fn is_usable_on_link(self, address: Ipv4Address) -> bool {
        is_unicast(address)
            && self.network.contains(address)
            && self.network.broadcast() != Some(address)
            && (self.network.prefix.value() >= 31 || address != self.network.address)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HostIpv4Config {
    interface: Ipv4InterfaceConfig,
    gateway: Option<Ipv4Address>,
}

impl HostIpv4Config {
    pub fn new(
        interface: Ipv4InterfaceConfig,
        gateway: Option<Ipv4Address>,
    ) -> Result<Self, ConfigurationError> {
        if let Some(ip) = gateway
            && (!interface.is_usable_on_link(ip) || ip == interface.address)
        {
            return Err(ConfigurationError::InvalidGateway);
        }
        Ok(Self { interface, gateway })
    }
    pub fn interface(self) -> Ipv4InterfaceConfig {
        self.interface
    }
    pub fn default_gateway(self) -> Option<Ipv4Address> {
        self.gateway
    }
}

impl Ipv4PrefixLength {
    pub fn new(value: u8) -> Result<Self, ConfigurationError> {
        if value > 32 {
            return Err(ConfigurationError::InvalidPrefixLength(value));
        }
        Ok(Self(value))
    }
    pub fn value(self) -> u8 {
        self.0
    }
    pub fn netmask_u32(self) -> u32 {
        if self.0 == 0 {
            0
        } else {
            u32::MAX << (32 - self.0)
        }
    }
}
