# 20: Congestion controller switch

**What to build:** `DATALINK_CONGESTION=<name>` on the Helper picks QUIC's congestion controller for every connection: `bbr`, `cubic` or `newreno`. Unset or empty keeps iroh's default, which is Cubic. An unknown value logs a warning and keeps the default. It's read once when the endpoint is built, beside `DATALINK_RELAY_ONLY` in `crates/datalink-transport/src/runtime.rs`. When set, it logs at start which controller is on. It ships undocumented for players, like `DATALINK_RELAY_ONLY`, and is named in the Patch B release notes for testers.

Why: in the [lossy Direct game](../analysis/lossy-direct-game.md), about 5% loss under load caused 56 congestion events in the game start. Cubic held the window at 2–3 packets, so even with Early acks the link moves about 29–48 KB/s. BBR estimates bandwidth instead of halving on each loss, so it should do better on random Wi-Fi loss. This is unmeasured; [19](19-captured-game-on-a-lossy-long-distance-link.md) measures it.

How, from the iroh 1.0.2 and noq-proto 1.0.1 sources:
- Build a `QuicTransportConfig` with `QuicTransportConfig::builder().congestion_controller_factory(Arc::new(...)).build()` and pass it to `Endpoint::builder(...).transport_config(...)`. The builder starts from iroh's own defaults, so nothing else changes. The endpoint uses that config for both dialled and accepted connections.
- `Bbr3Config`, `CubicConfig` and `NewRenoConfig` live in `noq_proto::congestion`, which iroh doesn't re-export. Add `noq-proto` as a direct dependency at the version iroh already uses, so the lock holds one copy.
- A controller governs only what its own Helper sends. In a two-player game the host sends nearly all the bytes, so the switch on the host alone gives most of the gain. It needs no Peer protocol change, and Helpers with different settings play together.

It's independent of Early acks and ships as Patch B's third opt-in switch ([ADR-0005](../../../docs/adr/0005-internet-play-speed-helpers-answer-acks.md), 2026-10-21 amendment). Ticket 18's flip to on by default doesn't include it. Whether it ever becomes a default is decided from 19's figures, in a later ticket.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] With the variable unset, the endpoint's transport config is unchanged
- [ ] `bbr`, `cubic` and `newreno` each select that controller; an unknown value warns and keeps the default (unit test on the parsing, as for `DATALINK_RELAY_ONLY`)
- [ ] An in-process two-Transport test plays messages over a connection with `bbr` on one side and the default on the other
- [ ] It logs at start which controller is on, when set
- [ ] `cargo tree -d` shows one `noq-proto`
