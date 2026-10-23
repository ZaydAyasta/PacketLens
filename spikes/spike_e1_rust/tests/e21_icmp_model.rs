use spike_e1_rust::*;

#[test]
fn error_summary_preserves_original() {
    for (id, sequence, destination) in [(7, 1, [198, 51, 100, 1]), (65535, 23, [203, 0, 113, 77])] {
        let original = Ipv4Packet {
            src_ip: Ipv4Address([192, 168, 1, 10]),
            dst_ip: Ipv4Address(destination),
            ttl: 1,
            payload: Ipv4Payload::Icmp(IcmpPacket {
                kind: IcmpKind::EchoRequest,
                identifier: id,
                sequence,
            }),
        };
        let before = original.clone();
        let summary = OriginalDatagramSummary::from_ipv4(&original);
        assert_eq!(summary.src_ip, original.src_ip);
        assert_eq!(summary.dst_ip, original.dst_ip);
        assert_eq!(summary.protocol, 1);
        assert_eq!(
            (summary.echo_identifier, summary.echo_sequence),
            (Some(id), Some(sequence))
        );
        let mut reply = original.clone();
        reply.payload = Ipv4Payload::Icmp(IcmpPacket {
            kind: IcmpKind::EchoReply,
            identifier: id,
            sequence,
        });
        assert_eq!(OriginalDatagramSummary::from_ipv4(&reply), summary);
        for (error, expected_type) in [
            (
                IcmpErrorPacket::TimeExceeded {
                    original: summary.clone(),
                },
                11,
            ),
            (
                IcmpErrorPacket::DestinationNetworkUnreachable {
                    original: summary.clone(),
                },
                3,
            ),
        ] {
            assert_eq!(error.code(), 0);
            assert_eq!(error.original(), &summary);
            assert_eq!(error.icmp_type(), expected_type);
            let outer = Ipv4Packet {
                src_ip: Ipv4Address([192, 168, 1, 1]),
                dst_ip: original.src_ip,
                ttl: 64,
                payload: Ipv4Payload::IcmpError(error),
            };
            let nested = OriginalDatagramSummary::from_ipv4(&outer);
            assert_eq!((nested.echo_identifier, nested.echo_sequence), (None, None));
            assert_eq!(original, before);
        }
    }
}
