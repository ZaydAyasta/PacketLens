//! Semantic ICMP errors. These summaries are not serialized network headers.
use crate::{IcmpKind, Ipv4Address, Ipv4Packet, Ipv4Payload};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OriginalDatagramSummary {
    pub src_ip: Ipv4Address,
    pub dst_ip: Ipv4Address,
    pub protocol: u8,
    pub echo_identifier: Option<u16>,
    pub echo_sequence: Option<u16>,
}
impl OriginalDatagramSummary {
    pub fn from_ipv4(packet: &Ipv4Packet) -> Self {
        let echo = match &packet.payload {
            Ipv4Payload::Icmp(icmp)
                if matches!(icmp.kind, IcmpKind::EchoRequest | IcmpKind::EchoReply) =>
            {
                Some(icmp)
            }
            _ => None,
        };
        Self {
            src_ip: packet.src_ip,
            dst_ip: packet.dst_ip,
            protocol: 1,
            echo_identifier: echo.map(|e| e.identifier),
            echo_sequence: echo.map(|e| e.sequence),
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IcmpErrorPacket {
    TimeExceeded { original: OriginalDatagramSummary },
    DestinationNetworkUnreachable { original: OriginalDatagramSummary },
    DestinationHostUnreachable { original: OriginalDatagramSummary },
}
impl IcmpErrorPacket {
    pub fn icmp_type(&self) -> u8 {
        match self {
            Self::TimeExceeded { .. } => 11,
            Self::DestinationNetworkUnreachable { .. }
            | Self::DestinationHostUnreachable { .. } => 3,
        }
    }
    pub fn code(&self) -> u8 {
        if matches!(self, Self::DestinationHostUnreachable { .. }) {
            1
        } else {
            0
        }
    }
    pub fn original(&self) -> &OriginalDatagramSummary {
        match self {
            Self::TimeExceeded { original }
            | Self::DestinationNetworkUnreachable { original }
            | Self::DestinationHostUnreachable { original } => original,
        }
    }
}
