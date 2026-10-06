"""Replay each Turn sync under a fix, keeping the game's compute and the causal order.

Model (first copies only):
- Each message m is sent by side S after S is 'unblocked': its previous reliable message is acked
  (stop-and-wait) and every message from the other side that S had drained before m (today) has been
  drained. send = unblock + g, where g (S's compute/reaction) is measured from today's timeline.
- Joiner's own send/receive times are inferred from host-side times with a one-way of RTT/2 and a
  far-end poll pJ.
- A message reaches the far Helper at tx_end + RTT/2, where tx_end serialises the sender's uplink at B
  bytes/s. Its ack (real) comes back after the far end's poll+react (ack_excess) + RTT/2.
- With a synthetic ack on side S, S's game gets the ack after c_local (the DLL poll) instead.
Calibration: with no fix the model should reproduce today's window length.
"""
import sys, collections, statistics as st
src = open(sys.argv[1]).read().split('# ---------------------------------------------------------------- report')[0]
sys.argv = [sys.argv[0], sys.argv[2]]
exec(src)

W = windows()

def prep(a, b):
    H, J = cycles(a, b)
    ev = []
    for h in H:
        ev.append(dict(side='H', t=h['t0'], ta=h['ta'], td=h['td'], size=h['size'], ty=h['ty'], r=h['rtt']))
    for j in J:
        r = rtt_at(j['ti'])
        ev.append(dict(side='J', t=j['ti'] - r / 2, ti=j['ti'], tdi=j['tdi'], tao=j['tao'], size=j['size'], ty=j['ty'], r=r))
    ev.sort(key=lambda e: e['t'])
    return ev

def sim(ev, a, b, rtt=None, synth=(), c_local=6000, pJ=6000, pH=None, B=120e3, comp=1.0, ack_excess=None):
    """Return the replayed end time - start. rtt: override RTT (us). synth: sides with synthetic acks.
    c_local: synthetic-ack latency to the game (us). pH/pJ: poll delay when a message reaches a game."""
    # measured per-message gaps g (today)
    last = {'H': None, 'J': None}          # previous own message of side
    recv = {'H': [], 'J': []}              # (today drain time at S, index) of other side's msgs
    G = []
    for i, e in enumerate(ev):
        r = e['r']
        if e['side'] == 'H':
            unb = [a]
            p = last['H']
            if p is not None: unb.append(ev[p]['td'])
            dep = [k for k, x in enumerate(ev[:i]) if x['side'] == 'J' and x['tdi'] <= e['t']]
            if dep: unb.append(ev[dep[-1]]['tdi'])
            g = max(0, e['t'] - max(unb))
        else:
            unb = [a - r / 2] if i == 0 else [a]
            p = last['J']
            if p is not None and ev[p]['tao']: unb.append(ev[p]['tao'] + ev[p]['r'] / 2 + pJ)
            dep = [k for k, x in enumerate(ev[:i]) if x['side'] == 'H' and x['t'] + x['r'] / 2 + pJ <= e['t']]
            if dep: unb.append(ev[dep[-1]]['t'] + ev[dep[-1]]['r'] / 2 + pJ)
            g = max(0, e['t'] - max(unb))
        last[e['side']] = i
        G.append((g, dep[-1] if dep else None))
    # replay
    T = {}; arr = {}; ackt = {}; free = {'H': -1e18, 'J': -1e18}
    last = {'H': None, 'J': None}
    for i, e in enumerate(ev):
        S = e['side']; g, dep = G[i]
        r = rtt if rtt else e['r']
        unb = [a if i else e['t']]
        p = last[S]
        if p is not None: unb.append(ackt[p])
        if dep is not None: unb.append(arr[dep] + (pH if (pH is not None and S == 'H') else (e.get('pollH', 6000) if S == 'H' else pJ)))
        s = max(unb) + g
        T[i] = s
        tx = max(s, free[S]) + e['size'] * comp / B * 1e6 if S == 'H' else s
        free[S] = tx
        arr[i] = tx + r / 2
        if S in synth:
            ackt[i] = s + c_local
        else:
            exc = 7000 if ack_excess is None else ack_excess   # far end poll + react, measured +5-8 ms
            pl = (pH if pH is not None else 6000) if S == 'H' else pJ
            ackt[i] = arr[i] + exc + r / 2 + pl
        last[S] = i
    endi = max(i for i, e in enumerate(ev) if e['side'] == 'H' and e['ty'] == '0x4309')
    return (max(T[endi], arr[endi] + pJ) - a) / 1e6

print(f"{'window':12} {'today':>6} {'model':>6} | {'synthH':>6} {'synthHJ':>7} {'+poll1':>6} {'+comp':>6} | {'poll1':>6} {'comp':>6} | relayed: {'model':>6} {'synthHJ':>7} {'+poll1+comp':>11}")
for name, a, b in W:
    ev = prep(a, b)
    D = (b - a) / 1e6
    base = sim(ev, a, b)
    sH = sim(ev, a, b, synth=('H',))
    sHJ = sim(ev, a, b, synth=('H', 'J'))
    sHJp = sim(ev, a, b, synth=('H', 'J'), c_local=1000, pJ=1000, pH=1000)
    sHJpc = sim(ev, a, b, synth=('H', 'J'), c_local=1000, pJ=1000, pH=1000, comp=0.1)
    p1 = sim(ev, a, b, pJ=1000, pH=1000, ack_excess=2000)
    c = sim(ev, a, b, comp=0.1)
    R = 146000
    rb = sim(ev, a, b, rtt=R)
    rs = sim(ev, a, b, rtt=R, synth=('H', 'J'))
    rsp = sim(ev, a, b, rtt=R, synth=('H', 'J'), c_local=1000, pJ=1000, pH=1000, comp=0.1)
    nH = sum(1 for e in ev if e['side'] == 'H'); nJ = len(ev) - nH
    turns = sum(1 for x, y in zip(ev, ev[1:]) if x['side'] != y['side'])
    print(f'{name:12} {D:6.1f} {base:6.1f} | {sH:6.1f} {sHJ:7.1f} {sHJp:6.1f} {sHJpc:6.1f} | {p1:6.1f} {c:6.1f} | relayed: {rb:6.1f} {rs:7.1f} {rsp:11.1f}   nH={nH} nJ={nJ} direction changes={turns}')
