//! DirectPlay GUIDs and GUID type definition

use serde::{Deserialize, Serialize};

/// A Windows GUID (Globally Unique Identifier)
#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct GUID {
    pub data1: u32,
    pub data2: u16,
    pub data3: u16,
    pub data4: [u8; 8],
}

impl GUID {
    pub const fn new(data1: u32, data2: u16, data3: u16, data4: [u8; 8]) -> Self {
        Self {
            data1,
            data2,
            data3,
            data4,
        }
    }

    pub const fn zeroed() -> Self {
        Self {
            data1: 0,
            data2: 0,
            data3: 0,
            data4: [0; 8],
        }
    }

    pub fn is_nil(&self) -> bool {
        self.data1 == 0
            && self.data2 == 0
            && self.data3 == 0
            && self.data4 == [0; 8]
    }

    /// Generate a random GUID
    pub fn new_random() -> Self {
        // Simple random GUID generation using timestamp and counter
        use std::time::{SystemTime, UNIX_EPOCH};
        static COUNTER: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        let counter = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        let data1 = now.as_secs() as u32 ^ counter;
        let data2 = (now.as_nanos() & 0xFFFF) as u16;
        let data3 = ((now.as_nanos() >> 16) & 0x0FFF) as u16 | 0x4000; // Version 4
        let mut data4 = [0u8; 8];
        data4[0] = ((now.as_nanos() >> 32) & 0x3F) as u8 | 0x80; // Variant
        data4[1] = (now.as_nanos() >> 40) as u8;
        data4[2] = (counter >> 8) as u8;
        data4[3] = counter as u8;
        data4[4] = (now.as_secs() >> 8) as u8;
        data4[5] = (now.as_secs() >> 16) as u8;
        data4[6] = (now.as_secs() >> 24) as u8;
        data4[7] = std::process::id() as u8;

        Self {
            data1,
            data2,
            data3,
            data4,
        }
    }
}

impl std::fmt::Debug for GUID {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{{{:08X}-{:04X}-{:04X}-{:02X}{:02X}-{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}}}",
            self.data1,
            self.data2,
            self.data3,
            self.data4[0],
            self.data4[1],
            self.data4[2],
            self.data4[3],
            self.data4[4],
            self.data4[5],
            self.data4[6],
            self.data4[7]
        )
    }
}

impl std::fmt::Display for GUID {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(self, f)
    }
}

/// Macro to define a GUID constant
#[macro_export]
macro_rules! define_guid {
    ($name:ident, $d1:expr, $d2:expr, $d3:expr, $d4:expr, $d5:expr, $d6:expr, $d7:expr, $d8:expr, $d9:expr, $d10:expr, $d11:expr) => {
        pub const $name: $crate::GUID = $crate::GUID::new(
            $d1,
            $d2,
            $d3,
            [$d4, $d5, $d6, $d7, $d8, $d9, $d10, $d11],
        );
    };
}

// ============================================================================
// DirectPlay CLSIDs (Class IDs for COM)
// ============================================================================

// CLSID_DirectPlay = {D1EB6D20-8923-11D0-9D97-00A0C90A43CB}
define_guid!(
    CLSID_DIRECTPLAY,
    0xD1EB6D20, 0x8923, 0x11D0, 0x9D, 0x97, 0x00, 0xA0, 0xC9, 0x0A, 0x43, 0xCB
);

// CLSID_DirectPlayLobby = {2FE8F810-B2A5-11D0-A787-0000F803ABFC}
define_guid!(
    CLSID_DIRECTPLAYLOBBY,
    0x2FE8F810, 0xB2A5, 0x11D0, 0xA7, 0x87, 0x00, 0x00, 0xF8, 0x03, 0xAB, 0xFC
);

// ============================================================================
// DirectPlay Interface IDs
// ============================================================================

// IID_IDirectPlay = {5454E9A0-DB65-11CE-921C-00AA006C4972}
define_guid!(
    IID_IDIRECTPLAY,
    0x5454E9A0, 0xDB65, 0x11CE, 0x92, 0x1C, 0x00, 0xAA, 0x00, 0x6C, 0x49, 0x72
);

// IID_IDirectPlay2 = {2B74F7C0-9154-11CF-A9CD-00AA006886E3}
define_guid!(
    IID_IDIRECTPLAY2,
    0x2B74F7C0, 0x9154, 0x11CF, 0xA9, 0xCD, 0x00, 0xAA, 0x00, 0x68, 0x86, 0xE3
);

// IID_IDirectPlay2A = {9D460580-A822-11CF-960C-0080C7534E82}
define_guid!(
    IID_IDIRECTPLAY2A,
    0x9D460580, 0xA822, 0x11CF, 0x96, 0x0C, 0x00, 0x80, 0xC7, 0x53, 0x4E, 0x82
);

// IID_IDirectPlay3 = {133EFE40-32DC-11D0-9CFB-00A0C90A43CB}
define_guid!(
    IID_IDIRECTPLAY3,
    0x133EFE40, 0x32DC, 0x11D0, 0x9C, 0xFB, 0x00, 0xA0, 0xC9, 0x0A, 0x43, 0xCB
);

// IID_IDirectPlay3A = {133EFE41-32DC-11D0-9CFB-00A0C90A43CB}
define_guid!(
    IID_IDIRECTPLAY3A,
    0x133EFE41, 0x32DC, 0x11D0, 0x9C, 0xFB, 0x00, 0xA0, 0xC9, 0x0A, 0x43, 0xCB
);

// IID_IDirectPlay4 = {0AB1C530-4745-11D1-A7A1-0000F803ABFC}
define_guid!(
    IID_IDIRECTPLAY4,
    0x0AB1C530, 0x4745, 0x11D1, 0xA7, 0xA1, 0x00, 0x00, 0xF8, 0x03, 0xAB, 0xFC
);

// IID_IDirectPlay4A = {0AB1C531-4745-11D1-A7A1-0000F803ABFC}
define_guid!(
    IID_IDIRECTPLAY4A,
    0x0AB1C531, 0x4745, 0x11D1, 0xA7, 0xA1, 0x00, 0x00, 0xF8, 0x03, 0xAB, 0xFC
);

// ============================================================================
// DirectPlay Lobby Interface IDs
// ============================================================================

// IID_IDirectPlayLobby = {AF465C71-9588-11CF-A020-00AA006157AC}
define_guid!(
    IID_IDIRECTPLAYLOBBY,
    0xAF465C71, 0x9588, 0x11CF, 0xA0, 0x20, 0x00, 0xAA, 0x00, 0x61, 0x57, 0xAC
);

// IID_IDirectPlayLobbyA = {26C66A70-B367-11CF-A024-00AA006157AC}
define_guid!(
    IID_IDIRECTPLAYLOBBYA,
    0x26C66A70, 0xB367, 0x11CF, 0xA0, 0x24, 0x00, 0xAA, 0x00, 0x61, 0x57, 0xAC
);

// IID_IDirectPlayLobby2 = {0194C220-A303-11D0-9C4F-00A0C905425E}
define_guid!(
    IID_IDIRECTPLAYLOBBY2,
    0x0194C220, 0xA303, 0x11D0, 0x9C, 0x4F, 0x00, 0xA0, 0xC9, 0x05, 0x42, 0x5E
);

// IID_IDirectPlayLobby2A = {1BB4AF80-A303-11D0-9C4F-00A0C905425E}
define_guid!(
    IID_IDIRECTPLAYLOBBY2A,
    0x1BB4AF80, 0xA303, 0x11D0, 0x9C, 0x4F, 0x00, 0xA0, 0xC9, 0x05, 0x42, 0x5E
);

// IID_IDirectPlayLobby3 = {2DB72490-652C-11D1-A7A8-0000F803ABFC}
define_guid!(
    IID_IDIRECTPLAYLOBBY3,
    0x2DB72490, 0x652C, 0x11D1, 0xA7, 0xA8, 0x00, 0x00, 0xF8, 0x03, 0xAB, 0xFC
);

// IID_IDirectPlayLobby3A = {2DB72491-652C-11D1-A7A8-0000F803ABFC}
define_guid!(
    IID_IDIRECTPLAYLOBBY3A,
    0x2DB72491, 0x652C, 0x11D1, 0xA7, 0xA8, 0x00, 0x00, 0xF8, 0x03, 0xAB, 0xFC
);

// ============================================================================
// Service Provider GUIDs
// ============================================================================

// DPSPGUID_IPX = {685BC400-9D2C-11CF-A9CD-00AA006886E3}
define_guid!(
    DPSPGUID_IPX,
    0x685BC400, 0x9D2C, 0x11CF, 0xA9, 0xCD, 0x00, 0xAA, 0x00, 0x68, 0x86, 0xE3
);

// DPSPGUID_TCPIP = {36E95EE0-8577-11CF-960C-0080C7534E82}
define_guid!(
    DPSPGUID_TCPIP,
    0x36E95EE0, 0x8577, 0x11CF, 0x96, 0x0C, 0x00, 0x80, 0xC7, 0x53, 0x4E, 0x82
);

// DPSPGUID_SERIAL = {0F1D6860-88D9-11CF-9C4E-00A0C905425E}
define_guid!(
    DPSPGUID_SERIAL,
    0x0F1D6860, 0x88D9, 0x11CF, 0x9C, 0x4E, 0x00, 0xA0, 0xC9, 0x05, 0x42, 0x5E
);

// DPSPGUID_MODEM = {44EAA760-CB68-11CF-9C4E-00A0C905425E}
define_guid!(
    DPSPGUID_MODEM,
    0x44EAA760, 0xCB68, 0x11CF, 0x9C, 0x4E, 0x00, 0xA0, 0xC9, 0x05, 0x42, 0x5E
);

// Our custom IROH service provider GUID
// DPSPGUID_IROH = {12345678-ABCD-EF01-2345-6789ABCDEF01}
define_guid!(
    DPSPGUID_IROH,
    0x12345678, 0xABCD, 0xEF01, 0x23, 0x45, 0x67, 0x89, 0xAB, 0xCD, 0xEF, 0x01
);

// ============================================================================
// IUnknown IID
// ============================================================================

// IID_IUnknown = {00000000-0000-0000-C000-000000000046}
define_guid!(
    IID_IUNKNOWN,
    0x00000000, 0x0000, 0x0000, 0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46
);

// IID_IClassFactory = {00000001-0000-0000-C000-000000000046}
define_guid!(
    IID_ICLASSFACTORY,
    0x00000001, 0x0000, 0x0000, 0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_guid_debug() {
        let guid = CLSID_DIRECTPLAY;
        let s = format!("{:?}", guid);
        assert_eq!(s, "{D1EB6D20-8923-11D0-9D97-00A0C90A43CB}");
    }

    #[test]
    fn test_guid_equality() {
        assert_eq!(CLSID_DIRECTPLAY, CLSID_DIRECTPLAY);
        assert_ne!(CLSID_DIRECTPLAY, CLSID_DIRECTPLAYLOBBY);
    }

    #[test]
    fn test_guid_nil() {
        assert!(GUID::zeroed().is_nil());
        assert!(!CLSID_DIRECTPLAY.is_nil());
    }
}
