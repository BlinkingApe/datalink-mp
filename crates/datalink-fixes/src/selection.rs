//! Which patches exist for which game binary.
//!
//! `AVAILABILITY` is the one table to extend when a patch gains addresses
//! for another binary. The patch modules themselves still hold their own
//! addresses and verify their original bytes before writing.

use crate::binary::GameBinary;

/// A patch or probe set the crate can install.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Patch {
    BlitFix,
    ClassicColors,
    BorderFix,
    DebugProbes,
}

impl Patch {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Patch::BlitFix => "backwards blit fix",
            Patch::ClassicColors => "classic-colors fix",
            Patch::BorderFix => "border recolor",
            Patch::DebugProbes => "debug probes",
        }
    }
}

/// Every patch, in the order they are applied.
pub(crate) const ALL: [Patch; 4] = [
    Patch::BlitFix,
    Patch::ClassicColors,
    Patch::BorderFix,
    Patch::DebugProbes,
];

/// The binaries each patch has addresses for.
const AVAILABILITY: [(Patch, &[GameBinary]); 4] = [
    (Patch::BlitFix, &[GameBinary::Smac]),
    (Patch::ClassicColors, &[GameBinary::Smac, GameBinary::AlienCrossfire]),
    (Patch::BorderFix, &[GameBinary::Smac]),
    (Patch::DebugProbes, &[GameBinary::Smac]),
];

/// What to do with one patch in the detected binary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Decision {
    /// Install it.
    Apply,
    /// No version of this patch exists for the detected binary.
    NotAvailable(GameBinary),
    /// The binary wasn't recognized, so nothing is touched.
    UnknownBinary,
}

/// Decide every patch for the detected binary (`None` = unrecognized).
pub(crate) fn plan(binary: Option<GameBinary>) -> Vec<(Patch, Decision)> {
    ALL.iter()
        .map(|&patch| {
            let decision = match binary {
                None => Decision::UnknownBinary,
                Some(b) => {
                    let available = AVAILABILITY
                        .iter()
                        .any(|(p, bins)| *p == patch && bins.contains(&b));
                    if available {
                        Decision::Apply
                    } else {
                        Decision::NotAvailable(b)
                    }
                }
            };
            (patch, decision)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smac_gets_every_patch_and_probe() {
        let plan = plan(Some(GameBinary::Smac));
        assert_eq!(plan.len(), 4);
        assert!(plan.iter().all(|(_, d)| *d == Decision::Apply));
    }

    #[test]
    fn alien_crossfire_gets_classic_colors_but_no_border_detour() {
        let plan = plan(Some(GameBinary::AlienCrossfire));
        assert_eq!(plan.len(), 4);
        for (patch, d) in plan {
            let expected = if patch == Patch::ClassicColors {
                Decision::Apply
            } else {
                Decision::NotAvailable(GameBinary::AlienCrossfire)
            };
            assert_eq!(d, expected, "{patch:?}");
        }
    }

    #[test]
    fn unknown_binary_gets_nothing() {
        let plan = plan(None);
        assert_eq!(plan.len(), 4);
        assert!(plan.iter().all(|(_, d)| *d == Decision::UnknownBinary));
    }

    #[test]
    fn every_patch_has_an_availability_entry() {
        for patch in ALL {
            assert!(AVAILABILITY.iter().any(|(p, _)| *p == patch), "{patch:?}");
        }
    }
}
