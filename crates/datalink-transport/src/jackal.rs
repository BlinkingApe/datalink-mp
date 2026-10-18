//! The one decoder for JACKAL's header, the game's own reliability layer.
//!
//! A game payload starts with an 8-byte JACKAL header, then the game's own
//! message. The capture, the Turn sync detector and (later) Early acks all
//! classify payloads through this decoder so they can't disagree.

/// The JACKAL fields at the front of a game payload, as `terranx.exe` lays them
/// out.
#[derive(Debug, PartialEq, Eq)]
pub struct Jackal {
    /// Wire offset 0: 4 = reliable data, 2 = ack.
    pub kind: u16,
    /// Wire offset 4: the sender's sequence number for a reliable send.
    pub seq: u32,
    /// Wire offset 8: the game's message type, for data only.
    pub game_type: Option<u16>,
    /// Wire offset 12: the sender's faction, for data only.
    pub faction: Option<u32>,
}

impl Jackal {
    /// Decode the header, or `None` for a payload shorter than it.
    pub fn decode(data: &[u8]) -> Option<Self> {
        let u16_at = |o: usize| data.get(o..o + 2).map(|b| u16::from_le_bytes([b[0], b[1]]));
        let u32_at =
            |o: usize| data.get(o..o + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]));
        let kind = u16_at(0)?;
        let seq = u32_at(4)?;
        // An ack's bytes after the header are never written, so only data has a
        // game message to read.
        let is_data = kind & 0x4 != 0;
        Some(Jackal {
            kind,
            seq,
            game_type: if is_data { u16_at(8) } else { None },
            faction: if is_data { u32_at(12) } else { None },
        })
    }

    /// Carries reliable data: any data bit is set (so the setup kind `0x14` too).
    pub fn carries_reliable_data(&self) -> bool {
        self.kind & 0x4 != 0
    }

    /// Is exactly reliable data, kind `0x0004`.
    pub fn is_exactly_reliable_data(&self) -> bool {
        self.kind == 0x0004
    }

    /// Is exactly an ack, kind `0x0002` (so not the setup kind `0x12`).
    pub fn is_exactly_ack(&self) -> bool {
        self.kind == 0x0002
    }

    /// Carries no data bit but the ack bit is set (so the setup kind `0x12` too).
    /// This is what the Turn sync detector treats as an ack.
    pub fn carries_ack(&self) -> bool {
        !self.carries_reliable_data() && self.kind & 0x2 != 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header(kind: u16) -> Jackal {
        let mut data = vec![0u8; 16];
        data[0..2].copy_from_slice(&kind.to_le_bytes());
        data[4] = 7;
        Jackal::decode(&data).unwrap()
    }

    #[test]
    fn decodes_a_data_message() {
        // kind 4, seq 7, then type 0x2303 from faction 3.
        let mut data = vec![4, 0, 0, 0, 7, 0, 0, 0, 0x03, 0x23, 0, 0, 3, 0, 0, 0];
        data.extend_from_slice(&[0; 28]);
        assert_eq!(
            Jackal::decode(&data),
            Some(Jackal { kind: 4, seq: 7, game_type: Some(0x2303), faction: Some(3) })
        );
    }

    #[test]
    fn an_ack_has_no_game_message() {
        let data = [2, 0, 0, 0, 7, 0, 0, 0, 0xaa, 0xbb, 0xcc, 0xdd];
        assert_eq!(
            Jackal::decode(&data),
            Some(Jackal { kind: 2, seq: 7, game_type: None, faction: None })
        );
    }

    #[test]
    fn a_short_payload_has_no_header() {
        assert_eq!(Jackal::decode(&[]), None);
        assert_eq!(Jackal::decode(&[4, 0, 0, 0, 7]), None);
        assert_eq!(Jackal::decode(&[4, 0, 0, 0, 7, 0, 0]), None);
    }

    #[test]
    fn classifications() {
        // (kind, carries data, exactly 0x0004, exactly 0x0002, carries ack)
        let table = [
            (0x0004u16, true, true, false, false),
            (0x0002, false, false, true, true),
            (0x0014, true, false, false, false),
            (0x0012, false, false, false, true),
            (0x0006, true, false, false, false),
            (0x0000, false, false, false, false),
            (0x0001, false, false, false, false),
        ];
        for (kind, data, exact_data, exact_ack, ack) in table {
            let j = header(kind);
            assert_eq!(j.carries_reliable_data(), data, "data {kind:#x}");
            assert_eq!(j.is_exactly_reliable_data(), exact_data, "exact data {kind:#x}");
            assert_eq!(j.is_exactly_ack(), exact_ack, "exact ack {kind:#x}");
            assert_eq!(j.carries_ack(), ack, "ack {kind:#x}");
        }
    }
}
