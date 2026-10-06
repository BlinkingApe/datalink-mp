# Research: SMAC's network layer (JACKAL) and its Turn sync

- **Date:** 2026-10-06
- **Question:** What can public sources tell us about JACKAL, the game's own network layer, and about the **Turn sync**? Specifically: how its reliability works (window, ack and retransmit timeouts, whole-message resends), what the game sends between turns, whether messages carry a header that names their kind, turn or sender, and whether anything is already compressed. Serves `.scratch/internet-play-speed` issue 02 and ADR-0004.
- **Method:** Public sources first: Thinker's source (the most complete public reverse-engineering of the game's turn and network code), OpenSMACX, PRACX's source, and Apolyton forum threads. None of them documents JACKAL's transport, so it was read from the code instead. I disassembled `terranx.exe` (Alien Crossfire v2.0, the build Thinker targets) with `objdump` and used Thinker's address map to name the functions. That part isn't a public write-up, but it's the binary every player has, and anyone can reproduce it (section 6). This repo's own notes were checked against both. Every claim is marked **[fact]** (read directly from a cited source or disassembly) or **[inference]** (my reading of what the facts imply).

## TL;DR

1. **Reliability is strict stop-and-wait. The window is one message.** Each reliable send blocks the game's thread until **every** recipient has acked that message, or until 20 s pass. Only then does the next message go out. Acks are 12-byte echoes of the header. The sender keeps one sequence counter for all recipients, and the receiver drops anything whose sequence number isn't higher than the last one it saw from that sender (§2).
2. **Retransmits resend the whole message** to each recipient that hasn't acked, with ×1.5 backoff. The per-peer timeout starts at DirectPlay's `DPCAPS.dwLatency`, which comes from **our DLL's `GetCaps`, currently hardcoded to 50 ms**. It then adapts as `(sample + old + 20 ms) / 2`. After 20 s the game shows "SEND MESSAGE TIME EXPIRED" (§2.3).
3. **Turn sync is a lockstep upkeep, not a full-state transfer.** Every machine runs the upkeep simulation itself. They exchange up to 8 phase barriers through the host, plus small per-base and per-leader updates, plus 21 checksums. Full sections of game state are resent only when a checksum doesn't match (§3).
4. **Messages are small, and every reliable one costs at least one round trip.** Control messages are 44 bytes on the wire, checksums 156 bytes, and state syncs at most 2096 bytes (2048-byte chunks). **[inference]** Turn sync time is therefore roughly (number of reliable messages × round trip to the slowest peer) plus compute. Latency dominates, not bytes.
5. **There is a usable header.** The 8-byte JACKAL header carries a kind word and a sequence number. After it, every game message starts with its type and **the sender's faction**. The barrier messages carry the upkeep phase and the turn number. The host announces whose turn it is with `0x4309` (§4).
6. **Compression: only the save-file transfer is compressed (zlib).** The game links zlib 1.0.2, but `compress()` and `uncompress()` each have exactly one caller: the file-transfer code (`0x0f0b`). Turn sync payloads are raw (§5).

---

## 1. Sources checked

| Source | What it gave |
|---|---|
| [Thinker][thinker] @ `f851165` (2026-09-28) | Decompiled rewrite of the multiplayer turn loop (`net_control_turn`, `net_upkeep`, `net_upkeep_phase`, `net_end_of_turn`, `next_player_turn`) in [`src/gameturn.cpp`][t-gameturn]; message types in [`src/net.cpp`][t-net]; names and addresses of `NetDaemon`, `AlphaNet`, `Net` and `message_*`/`synch_*` in [`src/engine.cpp`][t-engine] and [`src/engine.h`][t-engineh]; struct layouts in [`src/engine_win.h`][t-win] and [`src/engine_types.h`][t-types]. **No transport code:** the `Net::*` functions are named only. |
| [OpenSMACX][osmacx] @ `2f7f4e2` (2021) | `jackal_version_check` (JACKAL is a Firaxis library, version "10.10", [`src/general.cpp:341`][o-general]); nothing on networking beyond a DirectPlay unit-stacking fix in the README. |
| [PRACX][pracx] @ `06726f8` | Graphics/UI only. No networking code, and the README only says to enable DirectPlay. |
| Apolyton threads ([190603][apo-190603], [184125][apo-184125]) | User reports only (see §7). No technical write-ups. |
| Game data files `Script.txt`, `jackal.txt` (ship with every copy) | The sync box texts (`#UPKEEP0`–`#UPKEEP7`) and JACKAL's own error dialogs with their timeouts. |
| `terranx.exe` v2.0 disassembly (§6) | All transport facts in §2, sizes in §4, zlib callers in §5. |
| This repo: `docs/ARCHITECTURE.md`, `crates/dplayx`, `crates/smac-fixes`, `crates/dp-types`, `crates/iroh-transport` | Prior findings (send funnel, eligibility byte, sequence field, header offsets), cross-checked in §8. |

Searches for a public description of JACKAL's protocol (web, GitHub, Apolyton, CivFanatics) found **nothing public**. The search engine knows JACKAL only through this repo's README.

## 2. Reliability

### 2.1 Packet format on the wire **[fact]**

`Net::send` (`0x630080`), `Net::send_group` (`0x62EEA0`) and `Net::send_packet_type` (`0x62F8A0`) each allocate *payload + 12* bytes. They write an 8-byte header, copy the game's payload to offset 8, and pass the whole buffer to `IDirectPlay4::Send`:

| Wire offset | Size | Meaning |
|---|---|---|
| 0 | u16 | JACKAL kind word. Bit `0x4` = reliable data, bit `0x2` = ack. Reliable game data is exactly `0x0004` (`mov word [esi],4` at `0x6301ba`, `0x62efe3`). Other kinds the receive thread dispatches on: `0x01`, `0x10`, `0x20`, `0x100`, `0x200` (`0x632865`–`0x632931`). |
| 2 | u16 | not written |
| 4 | u32 | sequence number for reliable sends (`++[Net+0xe4]`), otherwise 0 (`0x630164`–`0x63017b`) |
| 8 | n | the game message (§4) |
| 8+n | 4 | allocated slack, never written. The length sent is n+12, and the receiver delivers n (`sub ecx,0xc` at `0x63285e`). |

**JACKAL sends with `dwFlags = 0`, not `DPSEND_GUARANTEED`.** Every `IDirectPlay4::Send` call on its reliable path pushes 0 as flags, and so do the acks. It falls back to `DPSEND_GUARANTEED` only when an internal flag (`Net+0xd8` bit `0x10000000`) is set (`0x63001d`–`0x630054`). That flag is cleared when `GetCaps` succeeds at connection setup (`0x62e3bf`), and I found no code that sets it. **[inference]** In practice JACKAL always does its own reliability, as `ARCHITECTURE.md` says.

### 2.2 Stop-and-wait, one sequence counter **[fact]**

Sending a reliable message, in `Net::send_group` (`0x62efdd`–`0x62f21e`). `Net::send` and `send_packet_type` repeat the same loop.

1. Mark every recipient slot as pending (`slot+0x15c = 1`). The 16 player slots start at `Net+0x154` and are `0x58` bytes each.
2. Call `IDirectPlay4::Send` once, to the group or player.
3. Loop: `WaitForMultipleObjects` on the network events, process incoming messages (`Net::handle_sys_msg`), and check each slot. The function **does not return until every pending slot is acked**, or until **20 000 ms** pass (`cmp eax,0x4e20` at `0x62f219`; `Net+0xe0 = 0x4e20` in the constructor at `0x62d834`).

The receive thread (`Net::internal_receive`, `0x6327E0`):

- **Reliable data** (`kind & 6 == 4`): look up the sender's slot. If `seq <= slot.last_seq` (`slot+0x158`), the message is a duplicate and is **not delivered**. Otherwise it records the sequence number and queues the payload for the game. **In both cases it acks:** it rewrites the kind word to `(kind & ~4) | 2` and sends the first **12 bytes** back to the sender (`0x632b4d`–`0x632bf4`).
- **Ack** (`kind & 6 == 2`): it clears the sender's pending flag **only if** `ack.seq == Net+0xe4`, the sequence number of the message currently being sent. When no slot is pending, it signals the waiting sender (`0x632c0a`–`0x632c9a`).

So:

- **The window is one message** **[fact]**. The sequence counter is a single counter shared by all recipients, and an ack only counts if it matches the latest sequence number.
- **A broadcast waits for the slowest recipient** **[fact]** (all slots must clear).
- **Dedupe is "higher than last seen"** **[fact]**. **[inference]** If the transport ever delivered a sender's later message before an earlier one, the earlier one would be silently dropped as a "duplicate". That is the same ordering trap `ARCHITECTURE.md` describes, now with a code-level reason.

### 2.3 Timeouts and retransmits **[fact]**

- **Initial per-peer retransmit timeout (RTO) = `DPCAPS.dwLatency`.** At connection setup (`Net::join_service`, `0x62e376`–`0x62e40b`), JACKAL calls `IDirectPlay4::GetCaps(&caps, DPGETCAPS_GUARANTEED)` into a buffer at `0x9be4c0` and copies the dword at `0x9be4d8` (offset `0x18` = `dwLatency`) into every slot's RTO (`slot+0x160`) and into the default for new players (`Net+0x6d4`). **If `dwLatency` is 0, it uses 100 ms.**
- **Our DLL answers `dwLatency = 50`** (`crates/dplayx/src/directplay.rs:1166`). So in our games the first retransmit fires 50 ms after a send.
- **Retransmit:** when `now − last_send > rto`, it sets `rto = rto × 3/2` and **resends the entire message** (same buffer, same length) to that one recipient with another `Send` (`0x62f191`–`0x62f1ed`).
- **RTO update on each ack:** `rto = (now − last_send + rto + 20) / 2` (`0x62f171`–`0x62f18d`). `last_send` is the time of the *latest* transmission, retransmits included.
- **Give up after 20 s** with the dialog `#NET_SENDTIMEDOUT`: "A message has failed to be transmitted to the following players… Try again for 20 seconds / Drop from session… / Exit game / Skip this message (potentially fatal)" (`jackal.txt`). The name list is built at `0x62f224`–`0x62f2a2`.
- **Liveness is separate:** `#NET_OKTODROPCLIENT` / `#NET_ADJUSTTOLERANCE` offer a drop tolerance of 5, **10 (default)**, 20 or 30 s (`jackal.txt`). Kind `0x20` packets refresh a per-peer "last heard" time (`slot+0x164`, `0x632909`–`0x632915`). **[inference]** That time drives the "PLAYER NOT RESPONDING" dialog.

**[inference] What this means over a slow or relayed connection.** Take an RTT of 250 ms. With an initial RTO of 50 ms, the first message is sent at t = 0 and resent at about 50, 125 and 237 ms before the ack arrives. That is up to 3 extra copies of a message of up to about 2 KB. The receiver drops them, but acks each one. The RTO then converges only to about (sample + RTO + 20)/2, and the sample is measured from the last retransmit. So it can settle *below* the true RTT and keep producing spurious resends. Every such copy also takes a turn in our single ordered stream per peer. The capture planned in issue 05 can count these: look for the same sender and sequence number seen more than once.

## 3. What the game sends between turns

Source: Thinker's rewrite of the multiplayer turn loop. Thinker calls its own `net_control_turn` from its game loop ([`game.cpp:397`][t-game397]), so in Thinker builds this code *is* what runs. The original's log strings ("Skipping upkeep synch", "Failed GAME checksum - resynch", "Client falling out of upkeep loop" …) are all present in `terranx.exe`. **[inference]** Thinker's version is a close translation, and PRACX builds run the original equivalent.

### 3.1 End of turn **[fact]**

[`net_end_of_turn`][t-eot] runs in one of two modes, chosen by `GameMoreRules & 0x10` (Thinker: "multiplayer simultaneous moves related", [`engine_enums.h:411`][t-enums]).

- **Bit clear:** each client sends `0x8301` and waits, showing `#FINISHINGTURN` ("Other players are finishing their turns..."). Once every faction has finished and no locks are held, the host sends `0x4301` (repo name: NEXT_TURN).
- **Bit set:** the host walks the turn order itself with `0x4309`, which carries the faction whose turn it is next ([`next_player_turn`][t-npt]). Each client compares `CurrentFaction` with its own faction (`not_my_turn`). **[inference]** So the bit set means one player moves at a time, and the bit clear means simultaneous moves.

### 3.2 Upkeep phases: the Turn sync **[fact]**

Then [`net_control_turn`][t-nct] runs `turn_upkeep()` and [`net_upkeep()`][t-upkeep]. Every machine runs the same upkeep (`faction_upkeep` for every faction, tech, prototype upgrades, the AI's `enemy_turn`) **locally**. The network only keeps the machines in step. Each phase ends with [`net_upkeep_phase(n)`][t-phase], a barrier:

- each client sends **`0x2303`** (`message_data(0x2303, 0, phase, CurrentTurn, 0, 0)`), which goes to the host (binary string: "From Client: DONE UPKEEP");
- the host waits until all factions have reported, then broadcasts **`0x4303`** (phase, turn) ("From Server: NEXT UPKEEP") and waits for acks.

The binary also contains "(Upkeep instruction for wrong turn!)" and "(Upkeep ack for wrong phase!)", so receivers check both fields.

The sync box is JACKAL's `NetMsg` popup. If a barrier takes longer than 1 s, it shows `UPKEEP<n>`. Its texts come from `Script.txt`:

| Phase | `Script.txt` label | Traffic besides the barrier ([`gameturn.cpp`][t-upkeep]) |
|---|---|---|
| 0 | Upkeep Phase: Preliminary | none (local `synch_radius` and `veh_promote`) |
| 1 | Upkeep Phase: Messages | none (local `faction_upkeep` for every faction) |
| 2 | Upkeep Phase: Synchronize Bases | `0x2305`. If the host flagged a need, each client sends `synch_base(i)` for **each of its own bases marked changed** and `synch_leader(own)`, then another barrier |
| 3 | Upkeep Phase: Tech Advancements | barrier only if a human faction's research changed |
| 4 | Upkeep Phase: Prototype Upgrades | council message `0x2600` from the host if a vote is pending. Barrier only if needed |
| 5 | Upkeep Phase: Move Computer-Controlled Units | `enemy_turn` for each AI faction. The messages it produces aren't visible in this function |
| 6 | Upkeep Phase: Checksums & Synchronization | each machine sends **`0x2307`** carrying **21 u32 checksums** (`message_big_data(0x2307, 0, MasterChecksum, 21)`; `GameChecksum` is 0x54 bytes: game, leaders, map, vehicles, bases, 16 map sections, [`engine_types.h:968`][t-types]) |
| 7 | Upkeep Phase: Resynchronize Mismatched Data | **only on mismatch:** the host resends the failing category with `NetDaemon::synch(…, 0x2101)`: GAME (types 0, 36), LEADERS (6, 7, 15 for each of 8 factions), MAP (4098, 5), VEHICLES (37, 18), BASES (38, 20) or one map section (48). Then a barrier |

So **Turn sync is mostly barriers plus deltas** **[fact]**. Whole sections of game state cross the network only after a checksum mismatch. **[inference]** On a healthy game, a Turn sync is a few dozen small reliable messages. Its length scales with the number of barriers, the number of changed bases, and the AI and upkeep compute on the slowest machine.

### 3.3 Order and routing **[fact]**

`NetDaemon::send_message` (`0x532940`) routes on the type's high bits, and **every route is a reliable send** (it passes flags `1` to `Net::send`):

- `& 0x4000`: broadcast to everyone; it first stamps `timeGetTime()` into the message at +8;
- `& 0x8000`: to the host, or from the host to a named player;
- `& 0x2000`: to the host, with a special case at `0x5329bf`;
- otherwise: to a named player.

So client reports (`0x2xxx`) go to the host, and the host's answers or broadcasts are `0x4xxx`. Examples: `0x2303`→`0x4303` and `0x2f04`→`0x4f04`. The repo's notes also name `0x2101`→`0x4101` for syncs, which I didn't verify. **[inference]** Every client-originated update costs two hops through the host. With stop-and-wait, each hop costs at least one round trip to the slowest recipient.

## 4. Message format and sizes

### 4.1 Game message header **[fact]**

`message_data` (`0x592EE0`), the 144-byte variant at `0x592F50`, and `message_big_data` (`0x592FF0`) all build the same header. Offsets are within the game message; add 8 for the wire.

| Offset | Size | Field |
|---|---|---|
| 0 | u16 | message type (e.g. `0x2303`) |
| 4 | u32 | **sender's faction** (`CurrentPlayerFaction` @`0x939284`, Thinker: "MapWin->cOwner") |
| 8 | u32 | `timeGetTime()` at build time (re-stamped for `0x4000` broadcasts) |
| 12 | u32 | `NetDaemon+0x1b2c` @`0x93e8bc`, which Thinker calls `deletelist.cur_type_id`: reset in upkeep phase 6 ([`gameturn.cpp:519`][t-519]), incremented at the end of each `net_end_of_turn` ([`:769`][t-769]). **Not** a turn number |
| 16 | 4×u32 | `message_data` arguments (32 bytes total), or up to 32 u32 for `message_big_data` (144 bytes total) |

`NetDaemon::synch` (`0x532E00`), used for the `0x2101` state syncs, uses a 0x24-byte header. It has the same type and faction fields, `+0x0c` = the same counter, `+0x10` = sync type (e.g. `0x11` vehicle, `0x13` base, `0x06` leader, from the `synch_*` wrappers at `0x593220`–`0x5934E0`), `+0x12` = chunk index, more arguments at `+0x14`–`+0x1c`, and data from `+0x24`. Data is split into **2048-byte chunks** (`shl esi,0xb` and `0x800` at `0x5333c3`–`0x53342a`).

### 4.2 Sizes on the wire (game message + 12) **[fact]**

| Message | Wire bytes |
|---|---|
| JACKAL ack | 12 |
| `message_data` (barriers `0x2303`/`0x4303`, `0x2305`, `0x8301`/`0x4301`, `0x4309`, diplomacy `0x244x`, unit actions `0x24xx`) | 44 |
| `message_big_data` (checksums `0x2307`) | 156 |
| `synch` state chunk (`0x2101`) | 0x24 + data ≤ 2048 → **≤ 2096** |
| save-file chunk (`0x0f0b`, compressed) | 0x20 + ≤ 2048 → ≤ 2092 |

### 4.3 Can a page read "whose turn"? **[fact, then inference]**

- The **sender's faction** is in every game message, at wire offset 12.
- The **turn number and upkeep phase** are the first two arguments of `0x2303`/`0x4303`, at wire offsets 24 and 28.
- In one-at-a-time mode, the host's **`0x4309` carries the faction whose turn it is**, at wire offset 24.
- In simultaneous mode, "who hasn't finished" is the set of human factions that haven't sent `0x8301` yet.

**[inference]** A Helper can read these reliably *if* it recognises reliable data by the kind word `0x0004` and ignores everything else (acks are 12 bytes with kind `0x0002`; duplicate retransmits repeat a sequence number it has already seen). Factions are numbers. Mapping a faction to a player name needs the faction pick (`0x2f04`/`0x4f04`, owner DPID at game +0x188 per the repo's decode) or the DPID→name roster the Helper already has.

## 5. Compression **[fact]**

- `terranx.exe` contains zlib 1.0.2 ("deflate 1.0.2 Copyright 1995-1996 Jean-loup Gailly", "inflate 1.0.2 … Mark Adler").
- zlib's `compress()` (`0x634280`: `deflateInit_(…, -1, "1.0.2", 0x38)` + `deflate(Z_FINISH)`) has **one caller**: `NetDaemon::send_files` (`0x52f2df`). That function maps a file, compresses it whole, and sends it in `0x820`-byte messages of type **`0x0f0b`** (repo name: SAVE_DATA) with chunk index at +0xc and data at +0x20.
- `uncompress()` (`0x634330`) has **one caller**: `NetDaemon::process_message` (`0x534c81`), which reassembles those chunks.
- `deflate`/`inflate` themselves are called only from those two wrappers. Save files on disk are raw or "encrypted", not zlib ([Thinker `savegame.cpp`][t-save] reads them with plain `fread`/`encrypt_read`).

So **only the save-file transfer when a game starts or reloads is compressed. Turn sync is not.** Synced structs (bases, vehicles, map sections) travel raw. **[inference]** They would compress well, but §2 and §3 suggest the time goes into round trips, not bytes. Compression between Helpers would shorten mostly the rare full resyncs and the first map sync, and barely touch the barrier-dominated normal Turn sync.

## 6. How to reproduce the disassembly

```
objdump -d -M intel --no-show-raw-insn terranx.exe > terranx.asm   # GNU objdump 2.41
```

`terranx.exe`: Alien Crossfire v2.0, sha256 `01901cbf7196b0c5d0df9540a029520f5df8fd9a6b343deef8b5663872805fcf`. Function names and addresses come from [Thinker `engine.cpp:3332-3414`][t-engine-net] (`AlphaNet_*`, `NetDaemon_*`, `Net_*`), `:1018-1038` (`message_*`, `synch_*`) and `:705-710` (turn functions). As a check that the build matches, the strings "Client waiting for acknowledgement" and "Awaiting acknowledgement of SYNCH" are referenced at `0x531322` and `0x5314ad`. Those addresses fall inside Thinker's `NetDaemon::await_exec` (`0x531300`) and `await_synch` (`0x531480`). IAT slots: `0x669368` timeGetTime, `0x669184` WaitForMultipleObjects; DirectPlay `Send` is vtable `+0x68`, `Receive` `+0x64`, `GetCaps` `+0x38`.

PRACX builds (`terran_PRACX.exe`, `terranx_PRACX.exe`) are separate binaries at different addresses. I didn't disassemble them. **[inference]** They contain the same JACKAL code: this repo's PRACX probes found the same shapes, namely `mov word [eax],4` for reliable sends, the player-count `>= 2` check at `+0x6dc`, the host DPID at `+0x764`, and the eligibility byte at slot `+0x168` bit `0x2` (§8).

## 7. Community reports (anecdotes, not evidence)

- "TCP/IP is quite buggy, but with simultaneous movement, it's almost unplayable." (Leon Trotsky, Apolyton, 2006-12-25, [thread 190603][apo-190603])
- Over Hamachi, "the game goes much more faster… especially when simoltaneous moves option is on" ([thread 184125][apo-184125]). No baseline is given.
- Thinker: "Simultaneous turns option for network multiplayer is known to cause issues. If the game desyncs, it is possible to save it to a file and host it again." ([Details.md:418][t-details])

## 8. Cross-check with this repo

| Repo claim | Verdict |
|---|---|
| JACKAL has its own sequence numbers, acks and retransmits (`docs/ARCHITECTURE.md`) | **Confirmed**, and now specified (§2). |
| "bytes 4-7 are a per-message SEQUENCE COUNTER" (`crates/dplayx/src/directplay.rs:1764`) | **Confirmed**: the JACKAL sequence number. |
| "The GAME message type sits at WIRE offset 8 (offset 0 is a routing category)" (`directplay.rs:1744`) | **Confirmed**: offset 0 is JACKAL's kind word. |
| Ready/lobby labels decode the type at wire offset **0** (`0x0f0b => SAVE_DATA`, `0x4301 => NEXT_TURN`, `directplay.rs:1733-1740`, `:1869-1876`) | **Conflicts** with the disassembly: offset 0 holds `0x0004` for reliable data. These labels should read offset 8. **To check in the capture.** |
| `DPLAYX_RXTRACE` reads `type` at wire 0, `hostSeq` at 8, `syncType` at 16, `chunkIdx` at 18, `dataOff` at 32 (`directplay.rs:359-386`, `:1826-1845`) | **Conflicts**: by §4.1, sync type and chunk index are at wire 24 and 26 (game +0x10 and +0x12), and wire 8 is the game type. **To check in the capture.** |
| Eligibility byte at slot `+0x168`, bit `0x2`; the reliable fan-out skips slots without it (`crates/dp-types/src/lib.rs:21-35`, `crates/iroh-transport/src/runtime.rs:452-470`) | **Matches** `terranx.exe`: `test byte [slot+0x168], 2` in the send loops (`0x62fa16`, `0x6301e5`). |
| Send funnel gates: multiplayer flag, player count `+0x6dc >= 2`, per-slot eligibility, then the "ack loop" (`crates/smac-fixes/src/lib.rs:305-318`) | **Matches**: `NetDaemon::send_message` checks `MultiplayerActive` (`0x93f660`), `Net::send` checks `+0x6dc >= 2` (`0x630139`), then the stop-and-wait loop of §2.2. **[inference]** The repo's PRACX names map to Thinker's names: RouteMessageByType ≈ `NetDaemon::send_message`, DirectPlaySendWrapper ≈ `Net::send`. |
| DLL `GetCaps` reports `dw_latency = 50` "estimated" (`directplay.rs:1166`) | **Matters more than it looks:** it is the game's initial retransmit timeout (§2.3). |

## 9. Open questions for the capture (issue 05)

1. Does every game `Send` arrive with `dwFlags = 0`? That would confirm the guaranteed-send fallback is never used.
2. How many reliable messages make up a normal Turn sync, and how many barriers? Compare with §3.2.
3. How many retransmits happen (same sender and sequence number seen more than once), and what is their spacing? It should follow 50 ms × 1.5ⁿ at first, then the adapted RTO.
4. What fraction of Turn sync time is round trips versus compute (gaps where no message is outstanding)?
5. Fix or confirm the header offsets in §8 rows 4–5 before any analysis depends on them.

[thinker]: https://github.com/induktio/thinker/tree/f851165e0df99d571f605e6fd8ec1092fbaca85a
[t-gameturn]: https://github.com/induktio/thinker/blob/f851165e0df99d571f605e6fd8ec1092fbaca85a/src/gameturn.cpp
[t-phase]: https://github.com/induktio/thinker/blob/f851165e0df99d571f605e6fd8ec1092fbaca85a/src/gameturn.cpp#L323-L365
[t-upkeep]: https://github.com/induktio/thinker/blob/f851165e0df99d571f605e6fd8ec1092fbaca85a/src/gameturn.cpp#L367-L617
[t-519]: https://github.com/induktio/thinker/blob/f851165e0df99d571f605e6fd8ec1092fbaca85a/src/gameturn.cpp#L519
[t-769]: https://github.com/induktio/thinker/blob/f851165e0df99d571f605e6fd8ec1092fbaca85a/src/gameturn.cpp#L769
[t-eot]: https://github.com/induktio/thinker/blob/f851165e0df99d571f605e6fd8ec1092fbaca85a/src/gameturn.cpp#L679-L770
[t-nct]: https://github.com/induktio/thinker/blob/f851165e0df99d571f605e6fd8ec1092fbaca85a/src/gameturn.cpp#L772-L917
[t-npt]: https://github.com/induktio/thinker/blob/f851165e0df99d571f605e6fd8ec1092fbaca85a/src/gameturn.cpp#L919-L931
[t-game397]: https://github.com/induktio/thinker/blob/f851165e0df99d571f605e6fd8ec1092fbaca85a/src/game.cpp#L397
[t-net]: https://github.com/induktio/thinker/blob/f851165e0df99d571f605e6fd8ec1092fbaca85a/src/net.cpp
[t-engine]: https://github.com/induktio/thinker/blob/f851165e0df99d571f605e6fd8ec1092fbaca85a/src/engine.cpp
[t-engine-net]: https://github.com/induktio/thinker/blob/f851165e0df99d571f605e6fd8ec1092fbaca85a/src/engine.cpp#L3332-L3414
[t-engineh]: https://github.com/induktio/thinker/blob/f851165e0df99d571f605e6fd8ec1092fbaca85a/src/engine.h
[t-win]: https://github.com/induktio/thinker/blob/f851165e0df99d571f605e6fd8ec1092fbaca85a/src/engine_win.h#L2701-L2880
[t-types]: https://github.com/induktio/thinker/blob/f851165e0df99d571f605e6fd8ec1092fbaca85a/src/engine_types.h#L968-L981
[t-enums]: https://github.com/induktio/thinker/blob/f851165e0df99d571f605e6fd8ec1092fbaca85a/src/engine_enums.h#L411
[t-save]: https://github.com/induktio/thinker/blob/f851165e0df99d571f605e6fd8ec1092fbaca85a/src/savegame.cpp#L45-L52
[t-details]: https://github.com/induktio/thinker/blob/f851165e0df99d571f605e6fd8ec1092fbaca85a/Details.md#L418
[osmacx]: https://github.com/b-casey/OpenSMACX/tree/2f7f4e24faddbaa5b97ee76d5e3dccfd855e4a8c
[o-general]: https://github.com/b-casey/OpenSMACX/blob/2f7f4e24faddbaa5b97ee76d5e3dccfd855e4a8c/src/general.cpp#L341-L351
[pracx]: https://github.com/DrazharLn/pracx/tree/06726f8914d58d9df4f543b508fc9246890d9e72
[apo-190603]: https://apolyton.net/forum/other-games/alpha-centauri/190603-problem-with-tcp-ip-multiplayer
[apo-184125]: https://apolyton.net/forum/other-games/alpha-centauri/ac-multiplaying/184125-playing-tcp-ip-over-hamachi-for-those-who-had-problems-starting-tcp-ip-over-inet
