#!/usr/bin/env python3
"""Analyse a traffic capture (docs/contributors/traffic-capture.md, format 1) for Turn sync timing.

Usage:  python3 analyse_capture.py <capture.jsonl> [peer]

Needs `zstandard`, `lz4` and `numpy` (pip install zstandard lz4 numpy).
The capturing Helper must be the host (the windows are found from the host's
0x4301 / 0x4303 / 0x4309 messages). Prints the numbers that
capture-analysis.md summarises.
"""
import bisect, collections, datetime, json, statistics as st, struct, sys, zlib

import lz4.block as lb
import numpy as np
import zstandard as zs

# ---------------------------------------------------------------- loading

L = [json.loads(l) for l in open(sys.argv[1])]
U0 = L[0]['unix_ms']


def clk(t):
    return datetime.datetime.fromtimestamp((U0 + t / 1000) / 1000).strftime('%H:%M:%S.%f')[:-3]


def u16(h, o): return struct.unpack_from('<H', bytes.fromhex(h), o)[0]
def u32(h, o): return struct.unpack_from('<I', bytes.fromhex(h), o)[0]


def args(d, n=4):
    b = bytes.fromhex(d['hex'])
    return [struct.unpack_from('<I', b, 24 + 4 * i)[0] for i in range(n) if 28 + 4 * i <= len(b)]


msgs = [d for d in L if d['ev'] in ('out', 'in')]
PEER = sys.argv[2] if len(sys.argv) > 2 else collections.Counter(
    d['peer'] for d in msgs if d['kind'] == 4).most_common(1)[0][0]
END = min([d['t_us'] for d in L if d['ev'] in ('closed', 'lost') and d.get('peer') == PEER] or [L[-1]['t_us']])
byid = {d['id']: d for d in msgs}
for d in L:
    if d['ev'] == 'written': byid[d['id']]['w'] = d['t_us']
    if d['ev'] == 'drain' and d['id'] in byid: byid[d['id']]['dr'] = d['t_us']
G = [d for d in msgs if d['peer'] == PEER and d['t_us'] < END]
DATA, ACK, HEARTBEAT = (4, 20), (2, 18), 32
stats = [d for d in L if d['ev'] == 'stats' and d['peer'] == PEER and d['t_us'] < END]
st_t = [s['t_us'] for s in stats]


def rtt_at(t):
    return stats[min(bisect.bisect_left(st_t, t), len(stats) - 1)]['rtt_us']


def first_copies(a=0, b=END):
    u = {}
    for d in G:
        if d['kind'] in DATA and a <= d['t_us'] <= b: u.setdefault((d['ev'], d['seq']), d)
    return sorted(u.values(), key=lambda d: d['t_us'])

# ---------------------------------------------------------------- windows


def windows():
    """Game start and each Turn sync: host's 0x4301 (or the game's start) to the next 0x4309."""
    U = first_copies()
    marks = [(d['t_us'], d['type']) for d in U if d['ev'] == 'out' and d.get('type') in ('0x4301', '0x4309')]
    start = next((d['t_us'] for d in L if d['ev'] == 'session_desc' and d['join_disabled']
                  and d['current_players'] >= 2), None)
    out = []
    if start is not None:
        out.append(('Game start', start, next(t for t, ty in marks if ty == '0x4309' and t > start)))
    n = 0
    for t, ty in marks:
        if ty == '0x4301':
            end = next((x for x, y in marks if y == '0x4309' and x > t), None)
            if end is None: continue
            # the Turn sync starts at the Joiner's end of turn (0x8301), when there is one
            pre = [d['t_us'] for d in U if d['ev'] == 'in' and d.get('type') == '0x8301' and t - 1e6 < d['t_us'] <= t]
            n += 1
            out.append((f'Turn sync {n}', pre[-1] if pre else t, end))
    return out

# ---------------------------------------------------------------- cycles and budget


def cycles(a, b):
    """Each reliable message with its ack. Host: first send t0, ack in ta, ack drained td.
    Joiner: first copy in ti, drained tdi, host's ack out tao."""
    M = [d for d in G if a <= d['t_us'] <= b + 3e6]
    first, acks = {}, {}
    for d in M:
        if d['kind'] in DATA: first.setdefault((d['ev'], d['seq']), []).append(d)
        if d['kind'] in ACK: acks.setdefault((d['ev'], d['seq']), []).append(d)
    H, J = [], []
    for (ev, seq), cp in first.items():
        t0 = cp[0]['t_us']
        if t0 > b: continue
        if ev == 'out':
            ak = [x for x in acks.get(('in', seq), []) if x['t_us'] > t0]
            if ak:
                H.append(dict(t0=t0, w=cp[0]['w'], ta=ak[0]['t_us'], td=ak[0]['dr'], size=cp[0]['size'],
                              copies=cp, ty=cp[0].get('type'), rtt=rtt_at(t0)))
        else:
            ak = [x for x in acks.get(('out', seq), []) if x['t_us'] > t0]
            J.append(dict(ti=t0, tdi=cp[0].get('dr', t0), tao=ak[0]['t_us'] if ak else None,
                          size=cp[0]['size'], copies=cp, ty=cp[0].get('type')))
    return sorted(H, key=lambda x: x['t0']), sorted(J, key=lambda x: x['ti'])


PRI = {'round trip': 0, 'DLL poll': 1, 'host reacts': 2, 'joiner compute': 3, 'host compute': 4}


def budget(a, b):
    """Label every instant of [a, b]: a host message awaiting its ack ('round trip'), a message
    sitting in the Helper until the DLL's poll drains it ('DLL poll'), the host's game acking
    ('host reacts'), or nothing in flight ('compute', credited to whichever side ends the gap)."""
    H, J = cycles(a, b)
    iv = []
    for h in H: iv += [(h['t0'], h['ta'], 'round trip'), (h['ta'], h['td'], 'DLL poll')]
    for j in J:
        iv.append((j['ti'], j['tdi'], 'DLL poll'))
        if j['tao']: iv.append((j['tdi'], j['tao'], 'host reacts'))
    iv = [(max(x, a), min(y, b), l) for x, y, l in iv if y > x and y > a and x < b]
    pts = sorted({a, b} | {x for x, _, _ in iv} | {y for _, y, _ in iv})
    tot, gaps = collections.Counter(), []
    for p, q in zip(pts, pts[1:]):
        cov = [l for x, y, l in iv if x <= p and y >= q]
        if cov: tot[min(cov, key=PRI.get)] += q - p
        elif gaps and gaps[-1][1] == p: gaps[-1][1] = q
        else: gaps.append([p, q])
    starts = [(h['t0'], 'host compute') for h in H] + [(j['ti'], 'joiner compute') for j in J]
    idle = []
    for p, q in gaps:
        who = min([s for s in starts if s[0] >= q - 1], default=(b, 'host compute'))[1]
        tot[who] += q - p
        idle.append((p, q, who))
    return tot, H, J, idle

# ---------------------------------------------------------------- report


def pct(v, D): return f'{v / 1e6:.2f}s ({100 * v / D:.0f}%)'


print(f'peer {PEER}, game messages until {clk(END)}')
paths = [d for d in L if d['ev'] == 'path' and d['peer'] == PEER]
for d in paths: print(f'  path {clk(d["t_us"])} {d["change"]} {d["route"]} {d["addr"]}')
r = [s['rtt_us'] / 1000 for s in stats if s['route'] == 'direct']
if r: print(f'  direct RTT median {st.median(r):.0f} ms, p90 {sorted(r)[9 * len(r) // 10]:.0f}, min {min(r):.0f}, max {max(r):.0f}')
r = [s['rtt_us'] / 1000 for s in stats if s['route'] == 'relayed']
if r: print(f'  relayed RTT median {st.median(r):.0f} ms, n={len(r)}')
print('guaranteed flags:', collections.Counter(d['guaranteed'] for d in G))
print('kinds:', collections.Counter((d['ev'], d['kind']) for d in G))

W = windows()
print('\n== Windows and budgets')
for name, a, b in W:
    D = b - a
    tot, H, J, idle = budget(a, b)
    U = first_copies(a, b)
    print(f'{name}: {clk(a)} -> {clk(b)} {D / 1e6:.1f}s')
    for ev, lab in (('out', 'host'), ('in', 'joiner')):
        u = [d for d in U if d['ev'] == ev]
        allc = [d for d in G if a <= d['t_us'] <= b and d['ev'] == ev and d['kind'] in DATA]
        print(f'   {lab} sent {len(u)} msgs / {sum(d["size"] for d in u)} B '
              f'({len(allc)} sends / {sum(d["size"] for d in allc)} B with resends); '
              f'types {dict(collections.Counter(d["type"] for d in u).most_common())}')
    print('   budget: ' + ', '.join(f'{k} {pct(v, D)}' for k, v in sorted(tot.items(), key=lambda x: -x[1])))
    for p, q, w in idle:
        if q - p > 300e3: print(f'     idle {clk(p)} -> {clk(q)} {(q - p) / 1e6:.2f}s, ended by {w.split()[0]}')
    ak = [(h['ta'] - h['t0']) / 1000 for h in H]
    q = [h['rtt'] / 1000 for h in H]
    print(f'   ack wait median {st.median(ak):.0f} ms vs QUIC RTT median {st.median(q):.0f} ms; '
          f'ack in->drain median {st.median([(h["td"] - h["ta"]) / 1000 for h in H]):.1f} ms; '
          f'out->written max {max((h["w"] - h["t0"]) / 1000 for h in H):.2f} ms')
    # barriers per phase
    rel, seen = [], set()
    for d in U:
        if d['ev'] == 'out' and d.get('type') == '0x4303': rel.append((args(d, 2), d['t_us']))
    bounds = [('end of turn -> phase 0', a, rel[0][1])] + [
        (f'-> phase {p[0]} release', rel[i - 1][1], t) for i, (p, t) in enumerate(rel) if i > 0] + [
        ('-> 0x4309', rel[-1][1], b)]
    print(f'   phases released (turn {rel[0][0][1]}): {[p[0] for p, _ in rel]}')
    for lab, x, y in bounds:
        t2, h2, _, _ = budget(x, y)
        print(f'     {lab:24} {(y - x) / 1e6:5.2f}s  {len(h2):3} host msgs  ' +
              ', '.join(f'{k} {v / 1e6:.2f}' for k, v in sorted(t2.items(), key=lambda z: -z[1]) if v > 5e4))

print('\n== Cost of bytes')
xs, ys, rs = [], [], []
for _, a, b in W:
    for h in cycles(a, b)[0]:
        xs.append(h['size'] / 1000); ys.append((h['ta'] - h['t0']) / 1000); rs.append(h['rtt'] / 1000)
x, y, rr = map(np.array, (xs, ys, rs))
c1 = np.linalg.lstsq(np.vstack([np.ones_like(x), x]).T, y, rcond=None)[0]
c2 = np.linalg.lstsq(np.vstack([np.ones_like(x), x, rr]).T, y, rcond=None)[0]
print(f'ack wait = {c1[0]:.0f} ms + {c1[1]:.1f} ms/KB;  with QUIC RTT as a regressor: {c2[1]:.1f} ms/KB')
for lo, hi in ((0, .064), (.064, 1.024), (2.0, 2.1)):
    m = (x >= lo) & (x < hi)
    print(f'  {lo * 1000:.0f}-{hi * 1000:.0f} B: n={m.sum()} ack wait median {np.median(y[m]):.0f} ms, QUIC RTT median {np.median(rr[m]):.0f}')

print('\n== Retransmits (whole game)')
H, J = cycles(0, END)
for X, lab in ((H, 'host'), (J, 'joiner')):
    c = collections.Counter(len(x['copies']) for x in X)
    ex = sum(len(x['copies']) - 1 for x in X)
    eb = sum((len(x['copies']) - 1) * x['size'] for x in X); tb = sum(x['size'] for x in X)
    print(f'{lab}: {len(X)} msgs, copies {dict(sorted(c.items()))}, {ex} resends, resent {eb} B on {tb} B ({100 * eb / tb:.0f}%)')
    sp = collections.defaultdict(list)
    for m in X:
        for i in range(1, len(m['copies'])): sp[i].append((m['copies'][i]['t_us'] - m['copies'][i - 1]['t_us']) / 1000)
    for i, v in sorted(sp.items()):
        v = sorted(v); print(f'   copy {i}->{i + 1}: n={len(v)} median {st.median(v):.0f} ms, p10 {v[len(v) // 10]:.0f}, p90 {v[9 * len(v) // 10]:.0f}')
print('host copies sent after the ack had reached the Helper:', sum(1 for h in H for c in h['copies'] if c['t_us'] > h['ta']))
print('first copy to:', collections.Counter(h['copies'][0]['to'] for h in H),
      ' resends to:', collections.Counter(c['to'] for h in H for c in h['copies'][1:]))
print('stop-and-wait violations (host, joiner):',
      sum(1 for i in range(len(H) - 1) if H[i + 1]['t0'] < H[i]['ta']),
      sum(1 for i in range(len(J) - 1) if J[i]['tao'] and J[i + 1]['ti'] < J[i]['tao']))

print('\n== Payloads (first copies, whole game)')
U = first_copies()
sz = [d['size'] for d in U]
print(f'{len(U)} msgs, {sum(sz)} B, max {max(sz)} B')
for lo, hi in ((0, 64), (64, 256), (256, 1024), (1024, 2097), (2097, 1 << 20)):
    s = [v for v in sz if lo <= v < hi]; print(f'  {lo}-{hi - 1} B: {len(s)} msgs, {sum(s)} B')


def per_msg(ds):
    z = zs.ZstdCompressor(level=3)
    return {'zstd-3': sum(len(z.compress(bytes.fromhex(d['hex']))) for d in ds),
            'lz4': sum(len(lb.compress(bytes.fromhex(d['hex']), store_size=False)) for d in ds),
            'deflate-6': sum(len(zlib.compress(bytes.fromhex(d['hex']), 6)) - 6 for d in ds)}


def stream(ds):
    """One compressor per direction, flushed after each message, as one ordered stream per friend would."""
    t = collections.Counter()
    for ev in ('out', 'in'):
        x = [bytes.fromhex(d['hex']) for d in ds if d['ev'] == ev]
        c = zlib.compressobj(6, zlib.DEFLATED, -15)
        t['deflate-6'] += sum(len(c.compress(b) + c.flush(zlib.Z_SYNC_FLUSH)) for b in x)
        z = zs.ZstdCompressor(level=3).compressobj()
        t['zstd-3'] += sum(len(z.compress(b) + z.flush(zs.COMPRESSOBJ_FLUSH_BLOCK)) for b in x)
        hist = b''
        for b in x:
            t['lz4'] += len(lb.compress(b, store_size=False, dict=hist[-65536:]) if hist else lb.compress(b, store_size=False))
            hist += b
    return t


for lab, ds in (('all', U), ('>=1024 B', [d for d in U if d['size'] >= 1024]), ('<1024 B', [d for d in U if d['size'] < 1024])):
    raw = sum(d['size'] for d in ds)
    print(f'[{lab}] raw {raw} B')
    print('   per message: ' + ', '.join(f'{k} {v} ({100 * v / raw:.0f}%)' for k, v in per_msg(ds).items()))
    print('   streamed:    ' + ', '.join(f'{k} {v} ({100 * v / raw:.0f}%)' for k, v in stream(ds).items()))


def key(d):
    """'Same kind': a state sync's (type, sync type, chunk, first argument), else (type, size)."""
    h = d['hex']; t = d.get('type')
    if t in ('0x4101', '0x2101') and d['size'] >= 32: return (d['ev'], t, u16(h, 24), u16(h, 26), u32(h, 28))
    return (d['ev'], t, d['size'])


def delta(ds, last):
    raw = nz = comp = hits = 0; z = zs.ZstdCompressor(level=3)
    for d in ds:
        b = bytes.fromhex(d['hex']); k = key(d); raw += len(b); p = last.get(k)
        if p is not None and len(p) == len(b): b2 = bytes(i ^ j for i, j in zip(b, p)); hits += 1
        else: b2 = b
        nz += sum(1 for v in b2 if v); comp += len(z.compress(b2)); last[k] = b
    return raw, hits, nz, comp


print('Delta (XOR with the previous message of the same kind, then zstd-3), per window:')
last = {}
delta([d for d in U if d['t_us'] < W[0][1]], last)
for i, (name, a, b) in enumerate(W):
    ds = [d for d in U if a <= d['t_us'] <= b]
    z = zs.ZstdCompressor(level=3); plain = sum(len(z.compress(bytes.fromhex(d['hex']))) for d in ds)
    raw, hits, nz, comp = delta(ds, last)
    print(f'  {name}: raw {raw} B, {hits}/{len(ds)} with a predecessor, nonzero after delta {nz} ({100 * nz / raw:.0f}%), '
          f'delta+zstd {comp} ({100 * comp / raw:.0f}%), zstd alone {plain} ({100 * plain / raw:.0f}%)')
    nxt = W[i + 1][1] if i + 1 < len(W) else END
    delta([d for d in U if b < d['t_us'] < nxt], last)

print('\n== Header checks')
for ev in ('out', 'in'):
    s = [d['seq'] for d in first_copies() if d['ev'] == ev]
    print(f'{ev}: new seqs strictly increasing: {all(p < q for p, q in zip(s, s[1:]))}')
print('faction at wire 12:', collections.Counter((d['ev'], d['faction']) for d in first_copies()
                                                 if d.get('type') not in ('0x4f02', '0x4f03', '0x4f04', '0x0f0d')))
print('sync types at wire 24 (0x4101/0x2101):', sorted({u16(d['hex'], 24) for d in first_copies()
                                                        if d.get('type') in ('0x4101', '0x2101')}))
cs = [(d['ev'], d['faction'], [u32(d['hex'], o) for o in range(24, 108, 4)]) for d in first_copies() if d.get('type') in ('0x2307', '0x4307')]
for i in range(0, len(cs) - 2, 3):
    j, h = cs[i][2], cs[i + 2][2]
    print(f'checksums {clk([d for d in first_copies() if d.get("type") == "0x2307"][i // 3]["t_us"])}: '
          f'mismatched slots {[k for k in range(21) if j[k] != h[k]]}')
