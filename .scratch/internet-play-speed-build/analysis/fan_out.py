#!/usr/bin/env python3
"""Who each Helper's reliable game messages went to, in a traffic capture (format 1).

Usage:  python3 fan_out.py <capture.jsonl> ...

For each capture, counts the distinct reliable payloads (kind 0x0004, resends
merged by payload) by how many friends they went to, with their bytes, and
tallies each payload's (to DPID, type). `to` 0 is a message for every player.
"""
import collections, json, sys

for fn in sys.argv[1:]:
    L = [json.loads(l) for l in open(fn)]
    out = [d for d in L if d['ev'] == 'out' and d['kind'] == 4]
    payloads = collections.defaultdict(list)
    for d in out:
        payloads[d['hex']].append(d)
    count, size = collections.Counter(), collections.Counter()
    for copies in payloads.values():
        n = len({d['peer'] for d in copies})
        count[n] += 1
        size[n] += copies[0]['size']
    kinds = collections.Counter((c[0]['to'], c[0].get('type')) for c in payloads.values())
    print(fn)
    print(f"  {len(payloads)} payloads; by friends sent to: {dict(count)}; bytes: {dict(size)}")
    print('  (to, type):', ', '.join(f'({t:#x}, {ty}) {n}' for (t, ty), n in kinds.most_common()))
