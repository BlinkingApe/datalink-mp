#!/usr/bin/env python3
"""Hop time, network time and the delivery check for a capture taken with Early acks.

Usage:  python3 early_ack_analysis.py <capture.jsonl> [peer]

Standard library only. analyse_capture.py prints the same report after its own.
Format: docs/contributors/traffic-capture.md (format 1).

Hop: at each change of direction from our messages to the friend's, the time from
the last of our messages' `written` (its frame in QUIC's send buffer) to the `in`
of the friend's next message. Network time is the hops plus the DLL polls (each
delivered `in` message's wait from `in` to `drain`).

Delivery check, per friend:
  1. one `real_ack` per `early_ack` (matched by seq);
  2. no gaps in the friend's reliable seqs (first copies of their kind 4 messages);
  3. every message handed to the game was drained exactly once. Real acks the
     Helper swallowed are never drained, so they are expected to have no `drain`.
"""
import collections
import json
import statistics
import sys


def load(path):
    with open(path) as f:
        return [json.loads(line) for line in f if line.strip()]


def peers(lines):
    return sorted({d['peer'] for d in lines if d['ev'] in ('out', 'in') and d.get('peer')})


def _index(lines):
    """Map message id -> its out/in/loop line, with `w` (written) and `dr` (drain) times added."""
    byid = {d['id']: dict(d) for d in lines if d['ev'] in ('out', 'in', 'loop')}
    for d in lines:
        if d['ev'] == 'written' and d['id'] in byid:
            byid[d['id']]['w'] = d['t_us']
        elif d['ev'] == 'drain' and d['id'] in byid:
            byid[d['id']].setdefault('drains', []).append(d['t_us'])
    return byid


def hops(lines, peer, start=None, end=None):
    """The hop and DLL-poll figures for one friend, optionally within [start, end] (t_us)."""
    msgs = sorted((d for d in _index(lines).values()
                   if d.get('peer') == peer and d['ev'] in ('out', 'in')), key=lambda d: d['t_us'])
    swallowed = _swallowed_ids(lines, peer)
    hop_us, polls = [], []
    last_out = None
    prev = None
    for d in msgs:
        if start is not None and not start <= d['t_us'] <= end:
            prev = None
            continue
        if d['ev'] == 'in' and prev is not None and prev['ev'] == 'out' and last_out is not None:
            hop_us.append(d['t_us'] - last_out)
        if d['ev'] == 'out' and 'w' in d:
            last_out = d['w']
        elif d['ev'] == 'out':
            last_out = None  # never written: no hop can start from it
        if d['ev'] == 'in' and d['id'] not in swallowed and d.get('drains'):
            polls.append(d['drains'][0] - d['t_us'])
        prev = d
    return {
        'hops': len(hop_us),
        'hop_us': sum(hop_us),
        'hop_median_us': statistics.median(hop_us) if hop_us else None,
        'dll_poll_us': sum(polls),
        'network_us': sum(hop_us) + sum(polls),
    }


def _swallowed_ids(lines, peer):
    """Ids of the `in` acks the Helper swallowed: every kind-2 `in` whose seq has a `real_ack`."""
    real = {d['seq'] for d in lines if d['ev'] == 'real_ack' and d['peer'] == peer}
    return {d['id'] for d in lines
            if d['ev'] == 'in' and d.get('peer') == peer and d.get('kind') == 2 and d.get('seq') in real}


def delivery_check(lines, peer):
    """Run the three checks. Returns {check name: (ok, detail)}."""
    out = {}

    early = collections.Counter(d['seq'] for d in lines if d['ev'] == 'early_ack' and d['peer'] == peer)
    real = collections.Counter(d['seq'] for d in lines if d['ev'] == 'real_ack' and d['peer'] == peer)
    missing = sorted(s for s in early if real[s] == 0)
    extra = sorted(s for s in real if real[s] > 1 or s not in early)
    dup_early = sorted(s for s, n in early.items() if n > 1)
    ok = not (missing or extra or dup_early)
    out['one real ack per early ack'] = (
        ok, f'{sum(early.values())} early acks, {sum(real.values())} real acks'
        + (f'; no real ack for seqs {missing}' if missing else '')
        + (f'; unmatched or repeated real acks for seqs {extra}' if extra else '')
        + (f'; early-acked twice: {dup_early}' if dup_early else ''))

    seqs = sorted({d['seq'] for d in lines
                   if d['ev'] == 'in' and d.get('peer') == peer and d.get('kind') == 4})
    gaps = [s for s in range(seqs[0], seqs[-1] + 1) if s not in set(seqs)] if seqs else []
    out["no gaps in the friend's seqs"] = (
        not gaps, f'{len(seqs)} seqs'
        + (f' ({seqs[0]}..{seqs[-1]})' if seqs else '')
        + (f'; missing {gaps}' if gaps else ''))

    swallowed = _swallowed_ids(lines, peer)
    idx = _index(lines)
    handed = [d for d in idx.values() if d['ev'] in ('in', 'loop')
              and (d.get('peer') == peer or d['ev'] == 'loop') and d['id'] not in swallowed]
    never = sorted(d['id'] for d in handed if not d.get('drains'))
    twice = sorted(d['id'] for d in handed if len(d.get('drains', [])) > 1)
    drained_swallowed = sorted(i for i in swallowed if idx[i].get('drains'))
    out['every message drained once'] = (
        not (never or twice or drained_swallowed),
        f'{len(handed)} messages for the game'
        + (f'; never drained: ids {never}' if never else '')
        + (f'; drained more than once: ids {twice}' if twice else '')
        + (f'; swallowed yet drained: ids {drained_swallowed}' if drained_swallowed else ''))
    return out


def _s(us):
    return f'{us / 1e6:.2f}s'


def report(lines, peer, windows=()):
    """Print the early ack figures for one friend. `windows` is [(name, start_us, end_us)]."""
    states = [d for d in lines if d['ev'] == 'early_acks' and d['peer'] == peer]
    for d in states:
        print(f'  early acks {"on" if d["on"] else "off"} ({d["why"]})')
    ea = [d for d in lines if d['ev'] == 'early_ack' and d['peer'] == peer]
    ra = [d for d in lines if d['ev'] == 'real_ack' and d['peer'] == peer]
    held = [d['withheld_us'] for d in ea if d['withheld_us']]
    print(f'  {len(ea)} early acks ({len(held)} withheld), {len(ra)} real acks swallowed, '
          f'{sum(1 for d in lines if d["ev"] == "resend_dropped" and d["peer"] == peer)} resends dropped, '
          f'{sum(d["count"] for d in lines if d["ev"] == "replay" and d["peer"] == peer)} frames replayed')
    if ra:
        rtt = sorted(d['rtt_us'] for d in ra)
        print(f'  real ack rtt median {rtt[len(rtt) // 2] / 1000:.0f} ms, max {rtt[-1] / 1000:.0f} ms')
    for name, a, b in [('whole capture', None, None)] + list(windows):
        h = hops(lines, peer, a, b)
        med = f', median {h["hop_median_us"] / 1000:.0f} ms' if h['hop_median_us'] is not None else ''
        print(f'  {name}: {h["hops"]} hops {_s(h["hop_us"])}{med}; DLL polls {_s(h["dll_poll_us"])}; '
              f'network time {_s(h["network_us"])}')
    results = delivery_check(lines, peer)
    for name, (ok, detail) in results.items():
        print(f'  delivery check - {name}: {"ok" if ok else "FAIL"} ({detail})')
    return all(ok for ok, _ in results.values())


def main(argv):
    lines = load(argv[1])
    wanted = [argv[2]] if len(argv) > 2 else peers(lines)
    ok = True
    for peer in wanted:
        print(f'peer {peer}')
        ok &= report(lines, peer)
    return 0 if ok else 1


if __name__ == '__main__':
    sys.exit(main(sys.argv))
