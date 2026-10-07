# Architecture

## The problem

Sid Meier's Alpha Centauri does LAN/internet multiplayer through DirectPlay, a
retired Windows COM API. Wine implements enough of DirectPlay for the game to
run, but its providers speak 1999 protocols (IPX broadcast, direct TCP with
open ports) that are useless across today's NATed internet.

The obvious modern fix — put a P2P library inside a replacement `dplayx.dll` —
does not work: tokio (which Iroh needs) cannot run under Wine at all. mio's
Windows backend requires `\Device\Afd`-based IOCP that Wine does not implement
(see [wine-compatibility.md](../players/wine-compatibility.md)). So the networking must
live *outside* Wine.

## The design

```
game (thinker.exe, terran_PRACX.exe or wtp.exe, 32-bit, in Wine)
  │  DirectPlay COM calls (CreatePlayer, Send, Receive, EnumSessions, …)
  ▼
DLL: dplayx.dll (crates/dplayx, 32-bit Windows, in Wine)
  │  length-prefixed binary IPC over localhost TCP (crates/datalink-ipc)
  ▼
Helper: datalink-mp (crates/datalink-mp, native)
  │  session/roster state + system-message synthesis (crates/datalink-transport)
  ▼
Iroh endpoint — one QUIC connection per peer, full mesh
```

### The DLL (`crates/dplayx`)

`dplayx.dll` implements the DirectPlay COM surface the game actually uses:
interface QueryInterface/AddRef/Release plumbing, `EnumConnections` (advertising the
"Iroh P2P" provider), `Initialize`, `EnumSessions`, `Open`, `CreatePlayer`,
`Send`, `Receive`, `GetPlayerData`/`SetPlayerData`, `GetPlayerName`/
`SetPlayerName`, `SetSessionDesc`, and the system-message envelope the game's
message pump expects. Export ordinals match the real dplayx.dll (see
`dplayx.def`).

Two details matter more than the rest:

- **Pointer fixups.** DirectPlay system messages (`DPMSG_CREATEPLAYERORGROUP`
  etc.) contain interior pointers. The Helper serializes them as offsets; the
  DLL rewrites them into absolute addresses in the buffer it hands the game.
- **Faithful semantics over cleverness.** The game's JACKAL network layer has
  its own reliability (sequence numbers, acks, retransmits) and its own
  loopback queue. The DLL does not echo sends back to the sender and does not
  reorder, dedupe, or retry — it is a dumb, ordered pipe. Every place the DLL
  tried to be smart turned out to fight the game.

### The IPC layer (`crates/datalink-ipc`)

Simple request/response over localhost TCP: the DLL is the client, the Helper
is the listener, one request at a time (the handshake carries the IPC version;
a DLL whose IPC version doesn't match the Helper's fails it). `Receive` is
polled by the game every frame, so the hot call is `ReceiveMessage` → drained
from the Helper's queue.

### The transport (`crates/datalink-transport`)

- **Full mesh.** The joiner connects to the host by Ticket; the host announces
  new players (with their Tickets) to everyone; existing players dial the
  newcomer directly. A broadcast is N-1 directed QUIC sends by the origin — no
  relaying, no double delivery.
- **One ordered stream per peer direction.** All messages to a peer are
  enqueued (synchronously — enqueue order is wire order) onto a single
  long-lived QUIC stream carrying a `SMAC` + Peer protocol version preamble and
  4-byte length-prefixed frames. The receive side decodes and applies frames
  strictly sequentially. This makes per-peer FIFO a structural property rather
  than a hope; an integration test drives 200 messages through two real
  endpoints and asserts exact order
  (`crates/datalink-transport/tests/mesh_networking.rs`).
- **Name-table at join.** The host answers a join with the complete player
  table — IDs, names, Tickets, and per-player data — applied atomically by the
  joiner before it processes anything else, mirroring real DirectPlay's
  name-table download. Player-data updates that arrive for a not-yet-known
  player are buffered and merged at registration instead of dropped: cross-peer
  arrival order is inherently unordered in a mesh, and the game's own handler
  for the equivalent message silently discards early updates.
- **Loud failures.** Send errors are logged and surfaced; a Peer protocol version
  mismatch kills the stream with an unmissable error. Silent drops of
  control-plane messages produce multi-day debugging sessions; this codebase
  chooses noise.

### Why the player-data byte matters (a JACKAL note)

The game registers each remote player with a one-byte "flags" value taken from
the player's DirectPlay data blob (bit 0 = is-host, bit 1 = eligible for
reliable delivery). Its reliable send path silently skips any player whose
byte lacks bit 1 — no error, no retry; the symptom is "joiner hangs in the
lobby forever." The transport therefore guarantees that byte's delivery three
ways: baked into the host-side registration at join, carried in the name-table
and roster, and propagated via data-update messages (with the buffering above).
This single byte was the hardest bug in the project.

### Why the host's game hears of a joiner late (a JACKAL note)

The host's game registers each player once, when it gets that player's
`CREATEPLAYERORGROUP`, under the long name `GetPlayerName` returns at that
moment. It never renames a player: its `SETPLAYERORGROUPNAME` handler does
nothing. Joiners copy every name from the host's roster. A joiner's
JoinRequest is sent at its Open, before its CreatePlayer names the player, so
the host's Helper holds the joiner back from its game until the joiner's first
name update (its CreatePlayer) arrives, and announces it then, already named,
which is the order real DirectPlay uses. Announcing the joiner at the
JoinRequest left its name blank for the whole game, on every machine.

### datalink-fixes (`crates/datalink-fixes`)

In-memory patches applied when the DLL loads into the game process:

- **Backwards-blit fix** — repairs a game drawing routine that crashes under
  Wine.
- **Probe system** — optional, env-gated (`SMAC_PROBE_LOG`) trampoline hooks
  that log when specific game code paths execute, without changing behavior.
  Probes write a probe-id byte to a ring buffer from pure asm; a background
  thread drains it to a timestamped log. This is how the send-path behavior
  above was established, and it remains the primary diagnostic tool: the
  installed probes partition the game's reliable-send funnel so that a failure
  names the gate that ate the message.

### datalink-mock-client (`crates/datalink-mock-client`)

An interactive CLI that drives the transport like the game would (create
session, join, send) — useful for exercising networking with no game or Wine
involved.

## Testing

`cargo test` runs unit tests plus the end-to-end mesh test (two real Iroh
endpoints over local networking). For live-game verification the project used
three parallel Wine instances with per-instance Helpers on distinct ports —
instrumented runs confirmed zero loss and exact per-peer ordering of the
game's state-sync traffic.
