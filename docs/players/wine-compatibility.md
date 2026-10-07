# Why the networking lives outside Wine

The obvious architecture — do the P2P networking directly inside the
replacement `dplayx.dll` — fails under Wine for two independent reasons,
discovered the hard way. This document records both, because anyone modifying
this project will be tempted to "simplify" the helper away.

## Blocker: tokio/mio cannot run under Wine

Creating a tokio UDP socket (`tokio::net::UdpSocket::from_std()`) inside Wine
fails with error 66 (`ERROR_BAD_DEV_TYPE`):

```
INFO netwatch: socket.bind() SUCCESS
INFO netwatch: set_nonblocking SUCCESS
INFO netwatch: tokio::net::UdpSocket::from_std()
DEBUG failed to bind: Bad device type. (os error 66)
```

mio's Windows backend implements async I/O via `\Device\Afd`-based IOCP. Wine
implements `\Device\Afd` only as an empty directory, so mio's registration
fails before any of our code runs. This is a known, closed-without-workaround
incompatibility: [mio issue #1444](https://github.com/tokio-rs/mio/issues/1444).

Since Iroh is tokio-based, no amount of patching at our level fixes this
inside Wine. Hence the split: the DLL is a thin synchronous TCP client
(`std::net` works fine under Wine), and all Iroh/tokio code runs in a native
helper process.

## Fixable: ECN socket options

Even before hitting the mio wall, QUIC sends fail under Wine because
`quinn-udp` sets ECN socket options Wine doesn't support:

| Socket option | Under Wine |
|---|---|
| `UdpSocket::bind`, `IP_DONTFRAGMENT`, `IP_PKTINFO`, `WSARecvMsg` | work |
| `IP_RECVECN` / `IPV6_RECVECN` | fail with 10042 (`WSAENOPROTOOPT`) |

This one is patchable (make ECN setup failures non-fatal in `quinn-udp`), but
it is moot given the blocker above: with the helper architecture, the QUIC
sockets only ever run natively, where ECN setup succeeds. It is documented
here because it is the *first* failure anyone re-attempting in-Wine networking
will hit, before reaching the mio wall.

## Error code reference

- **10042 (`WSAENOPROTOOPT`)** — socket option not supported (Wine's ECN gap)
- **66 (`ERROR_BAD_DEV_TYPE`)** — `\Device\Afd` IOCP registration failure (the
  mio blocker)
