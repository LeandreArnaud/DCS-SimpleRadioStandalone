//! Golden vectors copied from `DCS-SR-CommonTests/Network/UDPVoicePacketTests.cs`.

use srs_protocol::{FrequencySegment, ShortGuid, VoicePacket};

const GUID: &str = "ufYS_WlLVkmFPjqCgxz6GA";
const GUID_BYTES: [u8; 22] = [
    117, 102, 89, 83, 95, 87, 108, 76, 86, 107, 109, 70, 80, 106, 113, 67, 103, 120, 122, 54, 71, 65,
];

fn single_freq_packet(hops: u8) -> Vec<u8> {
    let mut v = vec![
        79, 0, // total length
        6, 0, // audio length
        10, 0, // frequency part length
        0, 1, 2, 3, 4, 5, // audio
        0, 0, 0, 0, 0, 0, 89, 64, // 100.0 Hz
        4,  // modulation
        0,  // encryption
        1, 0, 0, 0, // unit id
        1, 0, 0, 0, 0, 0, 0, 0, // packet number
        hops,
    ];
    v.extend_from_slice(&GUID_BYTES);
    v.extend_from_slice(&GUID_BYTES);
    v
}

fn multi_freq_packet() -> Vec<u8> {
    let mut v = vec![
        99, 0, 6, 0, 30, 0, 0, 1, 2, 3, 4, 5, //
        0, 0, 0, 128, 233, 235, 173, 65, 0, 0, // 251 MHz AM clear
        0, 0, 0, 0, 56, 156, 124, 65, 1, 0, // 30 MHz FM clear
        0, 0, 0, 128, 233, 235, 173, 65, 0, 1, // 251 MHz AM key 1
        1, 0, 0, 0, // unit id
        1, 0, 0, 0, 0, 0, 0, 0, // packet number
        254,
    ];
    v.extend_from_slice(&GUID_BYTES);
    v.extend_from_slice(&GUID_BYTES);
    v
}

#[test]
fn encode_initial_voice_packet() {
    let p = VoicePacket {
        audio: vec![0, 1, 2, 3, 4, 5],
        frequencies: vec![FrequencySegment {
            freq_hz: 100.0,
            modulation: 4,
            encryption: 0,
        }],
        unit_id: 1,
        packet_number: 1,
        retransmission_count: 4,
        original_client_guid: ShortGuid::parse(GUID).unwrap(),
        client_guid: ShortGuid::parse(GUID).unwrap(),
    };
    let bytes = p.encode().unwrap();
    assert_eq!(bytes.len(), 79);
    assert_eq!(bytes, single_freq_packet(4));
}

#[test]
fn decode_initial_voice_packet() {
    let p = VoicePacket::decode(&single_freq_packet(231)).unwrap();
    assert_eq!(p.client_guid.as_str(), GUID);
    assert_eq!(p.original_client_guid.as_str(), GUID);
    assert_eq!(p.audio, vec![0, 1, 2, 3, 4, 5]);
    assert_eq!(
        p.frequencies,
        vec![FrequencySegment {
            freq_hz: 100.0,
            modulation: 4,
            encryption: 0
        }]
    );
    assert_eq!(p.unit_id, 1);
    assert_eq!(p.packet_number, 1);
    assert_eq!(p.retransmission_count, 231);
}

#[test]
fn encode_multiple_frequency_voice_packet() {
    let p = VoicePacket {
        audio: vec![0, 1, 2, 3, 4, 5],
        frequencies: vec![
            FrequencySegment {
                freq_hz: 251_000_000.0,
                modulation: 0,
                encryption: 0,
            },
            FrequencySegment {
                freq_hz: 30_000_000.0,
                modulation: 1,
                encryption: 0,
            },
            FrequencySegment {
                freq_hz: 251_000_000.0,
                modulation: 0,
                encryption: 1,
            },
        ],
        unit_id: 1,
        packet_number: 1,
        retransmission_count: 254,
        original_client_guid: ShortGuid::parse(GUID).unwrap(),
        client_guid: ShortGuid::parse(GUID).unwrap(),
    };
    let bytes = p.encode().unwrap();
    assert_eq!(bytes.len(), 99);
    assert_eq!(bytes, multi_freq_packet());
}

#[test]
fn decode_multiple_frequency_voice_packet() {
    let p = VoicePacket::decode(&multi_freq_packet()).unwrap();
    assert_eq!(p.frequencies.len(), 3);
    assert_eq!(p.frequencies[0].freq_hz, 251_000_000.0);
    assert_eq!(p.frequencies[1].freq_hz, 30_000_000.0);
    assert_eq!(p.frequencies[1].modulation, 1);
    assert_eq!(p.frequencies[2].encryption, 1);
    assert_eq!(p.retransmission_count, 254);
    assert_eq!(p.encode().unwrap(), multi_freq_packet());
}

#[test]
fn roundtrip_property() {
    use proptest::prelude::*;
    proptest!(|(audio in proptest::collection::vec(any::<u8>(), 0..1500),
                freqs in proptest::collection::vec((any::<f64>(), any::<u8>(), any::<u8>()), 0..12),
                unit_id in any::<u32>(), pn in any::<u64>(), hops in any::<u8>())| {
        let p = VoicePacket {
            audio,
            frequencies: freqs.into_iter().map(|(f, m, e)| FrequencySegment { freq_hz: f, modulation: m, encryption: e }).collect(),
            unit_id,
            packet_number: pn,
            retransmission_count: hops,
            original_client_guid: ShortGuid::new_random(),
            client_guid: ShortGuid::new_random(),
        };
        let bytes = p.encode().unwrap();
        prop_assert_eq!(bytes.len(), p.encoded_len());
        let back = VoicePacket::decode(&bytes).unwrap();
        // NaN != NaN: compare bit patterns.
        prop_assert_eq!(back.audio.clone(), p.audio.clone());
        prop_assert_eq!(back.frequencies.len(), p.frequencies.len());
        for (a, b) in back.frequencies.iter().zip(&p.frequencies) {
            prop_assert_eq!(a.freq_hz.to_bits(), b.freq_hz.to_bits());
            prop_assert_eq!(a.modulation, b.modulation);
            prop_assert_eq!(a.encryption, b.encryption);
        }
        prop_assert_eq!(back.unit_id, p.unit_id);
        prop_assert_eq!(back.packet_number, p.packet_number);
        prop_assert_eq!(back.retransmission_count, p.retransmission_count);
        prop_assert_eq!(back.original_client_guid, p.original_client_guid);
        prop_assert_eq!(back.client_guid, p.client_guid);
    });
}
