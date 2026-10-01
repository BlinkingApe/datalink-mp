# datalink-mp

Peer-to-peer multiplayer for Sid Meier's Alpha Centauri: a replacement DirectPlay DLL inside the game forwards networking to a native helper that talks to other players over Iroh.

## Language

### Components

**DLL**:
The 32-bit replacement `dplayx.dll` loaded by the game; it implements DirectPlay and forwards everything to the Helper.
_Avoid_: plugin, shim

**Helper**:
The native `datalink-mp` process that owns the Iroh endpoint, session state and the player's Ticket.
_Avoid_: server, daemon, backend

**Ticket**:
The shareable string identifying one Helper's Iroh endpoint; a joiner pastes the host's Ticket to connect. A Ticket is new each time the Helper starts or is Stopped, so it must be shared again.
_Avoid_: invite code, key, address

**Game folder**:
The folder containing the game's executable (Thinker or PRACX); the DLL and the Helper live in it too.
_Avoid_: install dir, game directory

**Stop**:
Ending the Helper's current connections without quitting it; the Helper comes back with a new Ticket.
_Avoid_: reset, disconnect, leave

### Connections

**Direct connection**:
A connection between two Helpers whose traffic goes straight between the two players' machines.
_Avoid_: P2P connection (every connection is peer-to-peer)

**Relayed connection**:
A connection between two Helpers whose traffic passes through a relay server, because no direct connection could be made. It is slower than a direct connection.
_Avoid_: proxied connection, fallback connection

### Versions

**IPC version**:
The version of the DLL ↔ Helper handshake. A mismatch means the installed DLL doesn't match the running Helper.
_Avoid_: protocol version (ambiguous)

**Peer protocol version**:
The version of the Helper ↔ Helper wire protocol. A mismatch means two players are running different builds.
_Avoid_: protocol version (ambiguous), wire version

**Release version**:
The single user-facing version of a published build; the number players compare with each other. A change to the IPC version or Peer protocol version always changes the minor part, so players whose Release versions share a minor part can play together.
_Avoid_: build number
