//! SMAC runtime bug fixes
//!
//! This library patches bugs in the game at runtime by modifying code in memory.
//! Integrated into dplayx.dll - call `apply_all_patches()` from DllMain.
//!
//! ## Modules
//!
//! - `blit_fix`: Fixes crash in backwards blit drawing
//! - `classic_colors`: classic faction colors in multiplayer (on by
//!   default; disable with `SMAC_NO_CLASSIC_COLORS=1`)
//! - `border_fix`: border faction->seat translation used by `classic_colors`
//!   (also available standalone via `SMAC_BORDER_MATCH_UNITS=1`)
//! - `probe`: Non-invasive logging probes for debugging

#![allow(non_snake_case)]
#![cfg_attr(not(target_os = "windows"), allow(dead_code))]

mod blit_fix;
mod border_fix;
mod classic_colors;
pub mod probe;

pub use probe::{init_probes, install_probe, register_probe};

#[cfg(target_os = "windows")]
use tracing::{error, info, warn};
#[cfg(not(target_os = "windows"))]
use tracing::info;

/// Apply all patches
///
/// Call this from the host DLL's DllMain during DLL_PROCESS_ATTACH.
pub fn apply_all_patches() {
    info!("smac-fixes: applying patches...");

    #[cfg(target_os = "windows")]
    match unsafe { blit_fix::apply() } {
        Ok(()) => info!("Backwards blit fix applied successfully"),
        Err(e) => error!("Failed to apply backwards blit fix: {}", e),
    }

    #[cfg(target_os = "windows")]
    match unsafe { classic_colors::apply() } {
        Ok(()) => info!("Classic-colors fix processed"),
        Err(e) => error!("Failed to apply classic-colors fix: {}", e),
    }

    #[cfg(target_os = "windows")]
    match unsafe { border_fix::apply() } {
        Ok(()) => info!("Border-recolor option processed"),
        Err(e) => error!("Failed to apply border recolor: {}", e),
    }

    #[cfg(not(target_os = "windows"))]
    info!("smac-fixes: no patches to apply (non-Windows)");

    info!("smac-fixes: patching complete");
}

/// Install the diagnostic probe set.
///
/// These probes trace the game's multiplayer message flow — lobby faction
/// selection, the reliable-send funnel, player registration, game-start
/// sync, and the global-lock protocol — without changing behavior. Their
/// fire patterns localize a networking failure to a specific gate in the
/// game's own code (see the per-probe comments below).
///
/// Probes are installed unconditionally but log nothing unless the
/// SMAC_PROBE_LOG environment variable is set to a file path.
#[cfg(target_os = "windows")]
pub fn install_debug_probes() {
    info!("smac-fixes: installing debug probes...");

    // Initialize probe system (spawns background drain thread)
    if let Err(e) = init_probes() {
        warn!("Failed to initialize probe system: {}", e);
        return;
    }

    // Register probe messages
    let probe_c1 = register_probe("GameStartOrchestrator ENTERED");
    let probe_a1 = register_probe("HostGameStartHandler ENTERED");
    let probe_b1 = register_probe("HostGameStartHandler: JOINER path");
    let probe_b2 = register_probe("HostGameStartHandler: HOST path");
    let probe_d1 = register_probe("BroadcastRulesSync ENTERED");
    let probe_d2 = register_probe("BroadcastGameStateSync ENTERED");
    // E1 fires when RouteMessageByType re-addresses a 0xa000 conditional-routing
    // message to +0x764 because g_isHost/g_mpPhase != 1. On the host, +0x764 is
    // its own DPID, so the message self-loopbacks in DirectPlaySendWrapper
    // instead of reaching the DLL. During normal in-game play this is the
    // game's local-dispatch mechanism and fires constantly; it is diagnostic
    // when correlated with a broadcast that never left the machine.
    let probe_e1 = register_probe("RouteMessageByType: 0xa000 dest->self (g_mpPhase!=1)");
    // F1: a player was registered into the host's table (RegisterPlayerInternal).
    // Its ORDER in the probe log vs the Broadcast/Fanout probes reveals the
    // eligibility timing race — if the host broadcasts before a joiner registers,
    // that joiner's chunks are silently dropped by the fan-out.
    let probe_f1 = register_probe("RegisterPlayerInternal: player registered on host");
    // F2: the reliable-broadcast fan-out found an ELIGIBLE recipient (slot+0x168 & 2).
    // If this NEVER fires while broadcasts happen, the joiner isn't eligible and the
    // sync/rebroadcast is dropped. Presence during game-start = joiner eligible in time.
    let probe_f2 = register_probe("Fanout: eligible recipient found (+0x168 & 2)");
    // F3: the host rejected a global-lock request (BuildMessage32(0x1203) LOCK_FAILED).
    // Repeated F3 during gameplay is the "someone else is resolving a global action"
    // symptom (lock leak: holder +0xe0 never cleared).
    let probe_f3 = register_probe("Host sent LOCK_FAILED_GLOBAL (0x1203)");
    // MODE_SET: the live site that writes g_multiplayerGameMode (0x918470) in
    // LobbyDialogHandler @0x48b4b8. Mode semantics: 0=Host NEW game, 1=Join,
    // 2=Load saved, 3=Host existing — 0 is the normal value when hosting a new
    // game. A timing marker for when the lobby handler commits the mode.
    let probe_mode = register_probe("MODE_SET g_multiplayerGameMode written (LobbyDialogHandler)");
    // 0x48b4b8: mov dword [0x918470], edx = 89 15 70 84 91 00 (6 bytes)
    match unsafe {
        install_probe(0x48b4b8, 6, probe_mode, &[0x89, 0x15, 0x70, 0x84, 0x91, 0x00])
    } {
        Ok(()) => info!("Probe MODE_SET (g_multiplayerGameMode write) installed at 0x48b4b8"),
        Err(e) => warn!("Failed to install probe MODE_SET: {}", e),
    }

    // FSA-FSD: faction-selection handler discriminators. The joiner's faction pick
    // (msg 0x2f04) is handled in ProcessReceivedMessages case 0x2f04 (0x54cdd6).
    // FSB/FSC/FSD sit inside the g_multiplayerGameMode∈{2,3} SAVED-GAME validation
    // block; for a hosted NEW game (mode 0) the handler legitimately bypasses them,
    // applies via CopyPlayerSlotData, and ALWAYS calls RouteMessageByType with the
    // type flipped to 0x4f04. Fire patterns:
    //   FSA only, new game ..... NORMAL (apply + echo attempted; echo fate decided in
    //                            the send funnel — see RM_*/SW_* probes below)
    //   FSA+FSB[, FSC, FSD] .... loaded-game validation path (rejects A/B as before)
    //   FSA never fires ........ pick never reached the game handler (upstream DLL/routing)
    let probe_fsa = register_probe("FSA Faction handler ENTERED (0x2f04)");
    let probe_fsb = register_probe("FSB Faction reached DPID-ownership check");
    let probe_fsc = register_probe("FSC Faction DPID check PASSED (past reject A)");
    let probe_fsd = register_probe("FSD Faction APPLIED (past reject B) -> echo 0x4f04");

    // FSA @ 0x54cdd6: mov cx,[ebp]; mov eax,[ebp+4] = 66 8b 4d 00 8b 45 04 (7 bytes)
    match unsafe {
        install_probe(0x54cdd6, 7, probe_fsa, &[0x66, 0x8b, 0x4d, 0x00, 0x8b, 0x45, 0x04])
    } {
        Ok(()) => info!("Probe FSA (faction handler entry) installed at 0x54cdd6"),
        Err(e) => warn!("Failed to install probe FSA: {}", e),
    }

    // FSB @ 0x54ce36: mov esi,[0x82ed34] (g_playerDisplayData) = 8b 35 34 ed 82 00 (6 bytes)
    match unsafe {
        install_probe(0x54ce36, 6, probe_fsb, &[0x8b, 0x35, 0x34, 0xed, 0x82, 0x00])
    } {
        Ok(()) => info!("Probe FSB (faction reached DPID check) installed at 0x54ce36"),
        Err(e) => warn!("Failed to install probe FSB: {}", e),
    }

    // FSC @ 0x54ce59: mov ecx,[0x947e90] (player count) = 8b 0d 90 7e 94 00 (6 bytes)
    // Reached only when the DPID-ownership check did NOT reject.
    match unsafe {
        install_probe(0x54ce59, 6, probe_fsc, &[0x8b, 0x0d, 0x90, 0x7e, 0x94, 0x00])
    } {
        Ok(()) => info!("Probe FSC (faction DPID check passed) installed at 0x54ce59"),
        Err(e) => warn!("Failed to install probe FSC: {}", e),
    }

    // FSD @ 0x54cea5: lea esi,[eax+eax*2]; shl esi,5 = 8d 34 40 c1 e6 05 (6 bytes)
    // The apply path — reached only when neither reject fired.
    match unsafe {
        install_probe(0x54cea5, 6, probe_fsd, &[0x8d, 0x34, 0x40, 0xc1, 0xe6, 0x05])
    } {
        Ok(()) => info!("Probe FSD (faction applied) installed at 0x54cea5"),
        Err(e) => warn!("Failed to install probe FSD: {}", e),
    }

    // H1a/H1b: global-lock GRANTED (holder +0xe0 set) vs RELEASED (holder cleared).
    // If GRANTED persistently outnumbers RELEASED, the lock is LEAKED (a client got
    // the lock and never released it) — that's a real "resolving global action" bug.
    // If they balance, F3 is just normal contention and no lock fix is needed.
    let probe_h1a = register_probe("Global lock GRANTED (holder +0xe0 set)");
    let probe_h1b = register_probe("Global lock RELEASED (holder +0xe0 cleared)");

    // Install probes
    // C1: GameStartOrchestrator entry (0x5a43d0)
    // sub esp, 0x100 = 81 ec 00 01 00 00
    match unsafe {
        install_probe(
            0x5a43d0,
            6,
            probe_c1,
            &[0x81, 0xec, 0x00, 0x01, 0x00, 0x00],
        )
    } {
        Ok(()) => info!("Probe C1 (GameStartOrchestrator) installed at 0x5a43d0"),
        Err(e) => warn!("Failed to install probe C1: {}", e),
    }

    // A1: HostGameStartHandler entry (0x543a70)
    // push -1; push 0x67fb2b = 6a ff 68 2b fb 67
    match unsafe {
        install_probe(
            0x543a70,
            6,
            probe_a1,
            &[0x6a, 0xff, 0x68, 0x2b, 0xfb, 0x67],
        )
    } {
        Ok(()) => info!("Probe A1 (HostGameStartHandler) installed at 0x543a70"),
        Err(e) => warn!("Failed to install probe A1: {}", e),
    }

    // B1: JOINER path entry (0x543c8f)
    // mov eax, [0x949444] = a1 44 94 94 00
    match unsafe {
        install_probe(
            0x543c8f,
            5,
            probe_b1,
            &[0xa1, 0x44, 0x94, 0x94, 0x00],
        )
    } {
        Ok(()) => info!("Probe B1 (JOINER path) installed at 0x543c8f"),
        Err(e) => warn!("Failed to install probe B1: {}", e),
    }

    // B2: HOST path entry (0x543b5f)
    // mov edi, 0x9ba564 = bf 64 a5 9b 00
    match unsafe {
        install_probe(
            0x543b5f,
            5,
            probe_b2,
            &[0xbf, 0x64, 0xa5, 0x9b, 0x00],
        )
    } {
        Ok(()) => info!("Probe B2 (HOST path) installed at 0x543b5f"),
        Err(e) => warn!("Failed to install probe B2: {}", e),
    }

    // D1: BroadcastRulesSync entry (0x544380)
    // sub esp, 8; push esi; push 0x2101 = 83 ec 08 56 68 01 21 00 00 (9 bytes)
    match unsafe {
        install_probe(
            0x544380,
            9,
            probe_d1,
            &[0x83, 0xec, 0x08, 0x56, 0x68, 0x01, 0x21, 0x00, 0x00],
        )
    } {
        Ok(()) => info!("Probe D1 (BroadcastRulesSync) installed at 0x544380"),
        Err(e) => warn!("Failed to install probe D1: {}", e),
    }

    // D2: BroadcastGameStateSync entry (0x543600)
    // sub esp, 0x18; push esi; push 0x2101 = 83 ec 18 56 68 01 21 00 00 (9 bytes)
    match unsafe {
        install_probe(
            0x543600,
            9,
            probe_d2,
            &[0x83, 0xec, 0x18, 0x56, 0x68, 0x01, 0x21, 0x00, 0x00],
        )
    } {
        Ok(()) => info!("Probe D2 (BroadcastGameStateSync) installed at 0x543600"),
        Err(e) => warn!("Failed to install probe D2: {}", e),
    }

    // E1: RouteMessageByType 0xa000-branch dest overwrite (0x54797b)
    // mov eax, [esi+0x764] = 8b 86 64 07 00 00 (6 bytes). This is the exact
    // instruction that redirects the rules/state sync to the host itself.
    match unsafe {
        install_probe(
            0x54797b,
            6,
            probe_e1,
            &[0x8b, 0x86, 0x64, 0x07, 0x00, 0x00],
        )
    } {
        Ok(()) => info!("Probe E1 (RouteMessageByType 0xa000 dest->self) installed at 0x54797b"),
        Err(e) => warn!("Failed to install probe E1: {}", e),
    }

    // F1: RegisterPlayerInternal entry (0x64d020)
    // mov edx,[esp+8]; push ebx = 8b 54 24 08 53 (5 bytes)
    match unsafe {
        install_probe(0x64d020, 5, probe_f1, &[0x8b, 0x54, 0x24, 0x08, 0x53])
    } {
        Ok(()) => info!("Probe F1 (RegisterPlayerInternal) installed at 0x64d020"),
        Err(e) => warn!("Failed to install probe F1: {}", e),
    }

    // F2: DirectPlaySendWrapper fan-out eligible-recipient path (0x64a6c2)
    // mov ebp,[esp+0x24]; mov [eax-4],edi = 8b 6c 24 24 89 78 fc (7 bytes)
    // (both relocated insns read esp/eax which are restored before they run)
    match unsafe {
        install_probe(0x64a6c2, 7, probe_f2, &[0x8b, 0x6c, 0x24, 0x24, 0x89, 0x78, 0xfc])
    } {
        Ok(()) => info!("Probe F2 (fan-out eligible recipient) installed at 0x64a6c2"),
        Err(e) => warn!("Failed to install probe F2: {}", e),
    }

    // F3: LOCK_FAILED_GLOBAL send in ProcessReceivedMessages (0x54acba)
    // push 0x1203 = 68 03 12 00 00 (5 bytes)
    match unsafe {
        install_probe(0x54acba, 5, probe_f3, &[0x68, 0x03, 0x12, 0x00, 0x00])
    } {
        Ok(()) => info!("Probe F3 (LOCK_FAILED_GLOBAL) installed at 0x54acba"),
        Err(e) => warn!("Failed to install probe F3: {}", e),
    }

    // ========================================================================
    // SEND-FUNNEL probes. The game's reliable sends (0x4f04 faction echo,
    // 0x2f02 lobby update, 0x2101 rules sync) can die silently inside the
    // game before ever reaching the DLL. A reliable send must pass, in order:
    //   gate 1: RouteMessageByType (0x5478c0) top: g_multiplayerActive!=0
    //   gate 2: DirectPlaySendWrapper dest==0 pre-check: playerCountThreshold(+0x6dc)>=2
    //   gate 3: per-slot eligibility (slot+0x14 & 2) in the fan-out (broadcast)
    //           or the directed-slot match loop (directed sends)
    // Fire-pattern verdict for the 0x4f04 echo (each FSA ⇒ one RouteMessageByType call):
    //   FSA, no RM_ENTER ........... handler never routed (shouldn't happen; re-disasm)
    //   RM_ENTER, no RM_4000 ....... gate 1: g_multiplayerActive==0 (find who zeroed it)
    //   RM_4000 + SW_BCAST_PRE,
    //     no SW_RELIABLE ........... gate 2: playerCountThreshold < 2
    //   SW_RELIABLE, no F2 ......... gate 3: no eligible slot (byte&2 clear / DPID miss)
    //   F2 fires, DLL silent ....... drop is past the fan-out (ack loop / vtable / DLL RX)
    //   SW_DIR_ELIG ................ a DIRECTED reliable send found its slot (will transmit)
    // DATA_0x102 / DATA_APPLIED close the loop on WHY a slot byte may be missing:
    // the DLL FIXUP log shows 0x102 delivery; DATA_0x102 without DATA_APPLIED means
    // the game skipped the byte write (DPID not yet registered = create/data arrival
    // race, or null lpData = fixup bug). Both silent-skip, no retry — session-permanent.
    let probe_rm_enter = register_probe("RM_ENTER RouteMessageByType called");
    let probe_rm_4000 = register_probe("RM_4000 gate passed; 0x4000-branch send -> wrapper");
    let probe_sw_bcast = register_probe("SW_BCAST_PRE dest=0 pre-check reached (count test next)");
    let probe_sw_rel = register_probe("SW_RELIABLE reliable path entered (marker=4)");
    let probe_sw_dir = register_probe("SW_DIR_ELIG directed reliable send matched eligible slot");
    let probe_data_entry = register_probe("DATA_0x102 SETPLAYERORGROUPDATA case entered");
    let probe_data_applied = register_probe("DATA_APPLIED eligibility byte written to slot");

    // RM_ENTER @ 0x5478c0: mov eax,[0x949308] (g_multiplayerActive) = a1 08 93 94 00 (5)
    match unsafe { install_probe(0x5478c0, 5, probe_rm_enter, &[0xa1, 0x08, 0x93, 0x94, 0x00]) } {
        Ok(()) => info!("Probe RM_ENTER installed at 0x5478c0"),
        Err(e) => warn!("Failed to install probe RM_ENTER: {}", e),
    }

    // RM_4000 @ 0x5478e3: mov [edi+8],eax; mov eax,[esp+0x10] = 89 47 08 8b 44 24 10 (7)
    // (first insn past the timeGetTime call inside the &0x4000 broadcast branch)
    match unsafe {
        install_probe(0x5478e3, 7, probe_rm_4000, &[0x89, 0x47, 0x08, 0x8b, 0x44, 0x24, 0x10])
    } {
        Ok(()) => info!("Probe RM_4000 installed at 0x5478e3"),
        Err(e) => warn!("Failed to install probe RM_4000: {}", e),
    }

    // SW_BCAST_PRE @ 0x64a2c7: cmp dword [ebx+0x6dc], 2 = 83 bb dc 06 00 00 02 (7)
    // (dest==0 + loopback-flag path, right after LoopbackQueueEnqueue; the relocated
    // cmp sets FLAGS for the jl at 0x64a2ce, which stays in place — trampoline's
    // trailing jmp does not touch FLAGS)
    match unsafe {
        install_probe(0x64a2c7, 7, probe_sw_bcast, &[0x83, 0xbb, 0xdc, 0x06, 0x00, 0x00, 0x02])
    } {
        Ok(()) => info!("Probe SW_BCAST_PRE installed at 0x64a2c7"),
        Err(e) => warn!("Failed to install probe SW_BCAST_PRE: {}", e),
    }

    // SW_RELIABLE @ 0x64a340: mov word [eax], 4 = 66 c7 00 04 00 (5)
    match unsafe { install_probe(0x64a340, 5, probe_sw_rel, &[0x66, 0xc7, 0x00, 0x04, 0x00]) } {
        Ok(()) => info!("Probe SW_RELIABLE installed at 0x64a340"),
        Err(e) => warn!("Failed to install probe SW_RELIABLE: {}", e),
    }

    // SW_DIR_ELIG @ 0x64a378: mov esi,[eax-8]; mov [esp+0x20],ecx = 8b 70 f8 89 4c 24 20 (7)
    // (inside the directed-send slot loop, only reached when slot DPID==dest AND byte&2)
    match unsafe {
        install_probe(0x64a378, 7, probe_sw_dir, &[0x8b, 0x70, 0xf8, 0x89, 0x4c, 0x24, 0x20])
    } {
        Ok(()) => info!("Probe SW_DIR_ELIG installed at 0x64a378"),
        Err(e) => warn!("Failed to install probe SW_DIR_ELIG: {}", e),
    }

    // DATA_0x102 @ 0x64d2c9: mov edx,[0x9c49c4] = 8b 15 c4 49 9c 00 (6)
    // (branch target of the 0x102 dispatch je — probe JMP sits at the target, fine)
    match unsafe {
        install_probe(0x64d2c9, 6, probe_data_entry, &[0x8b, 0x15, 0xc4, 0x49, 0x9c, 0x00])
    } {
        Ok(()) => info!("Probe DATA_0x102 installed at 0x64d2c9"),
        Err(e) => warn!("Failed to install probe DATA_0x102: {}", e),
    }

    // DATA_APPLIED @ 0x64d2fb: mov [ebx+eax*8+0x168], cl = 88 8c c3 68 01 00 00 (7)
    match unsafe {
        install_probe(0x64d2fb, 7, probe_data_applied, &[0x88, 0x8c, 0xc3, 0x68, 0x01, 0x00, 0x00])
    } {
        Ok(()) => info!("Probe DATA_APPLIED installed at 0x64d2fb"),
        Err(e) => warn!("Failed to install probe DATA_APPLIED: {}", e),
    }

    // H1a: global lock granted — CheckGlobalLockAvailable holder-set (0x5a6f3a)
    // mov [ecx+0xe0], edx = 89 91 e0 00 00 00 (6 bytes)
    match unsafe {
        install_probe(0x5a6f3a, 6, probe_h1a, &[0x89, 0x91, 0xe0, 0x00, 0x00, 0x00])
    } {
        Ok(()) => info!("Probe H1a (lock granted) installed at 0x5a6f3a"),
        Err(e) => warn!("Failed to install probe H1a: {}", e),
    }

    // H1b: global lock released — ReleaseLockOnServer holder-clear (0x5a6df4)
    // mov [ecx+0xe0], eax = 89 81 e0 00 00 00 (6 bytes)
    match unsafe {
        install_probe(0x5a6df4, 6, probe_h1b, &[0x89, 0x81, 0xe0, 0x00, 0x00, 0x00])
    } {
        Ok(()) => info!("Probe H1b (lock released) installed at 0x5a6df4"),
        Err(e) => warn!("Failed to install probe H1b: {}", e),
    }

    info!("smac-fixes: debug probes installation complete");
}

/// Stub for non-Windows builds
#[cfg(not(target_os = "windows"))]
pub fn install_debug_probes() {
    info!("smac-fixes: debug probes not available (non-Windows)");
}
