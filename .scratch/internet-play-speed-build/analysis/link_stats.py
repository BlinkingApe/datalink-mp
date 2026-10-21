#!/usr/bin/env python3
"""Loss and congestion window per window of a traffic capture (format 1).

Usage:  python3 link_stats.py <capture.jsonl> <name>=<HH:MM:SS>-<HH:MM:SS> ...

Reads the per-second `stats` lines on the Direct route. For each named window
(local clock times, as analyse_capture.py prints them) it prints datagrams
sent, packets lost on the path, loss rate, congestion events, and the
congestion window in packets (cwnd / mtu).
"""
import datetime, json, statistics as st, sys

L = [json.loads(l) for l in open(sys.argv[1])]
U0 = L[0]['unix_ms']


def clk(t):
    return datetime.datetime.fromtimestamp((U0 + t / 1000) / 1000).strftime('%H:%M:%S')


stats = [d for d in L if d['ev'] == 'stats' and d['route'] == 'direct']
for arg in sys.argv[2:]:
    name, span = arg.split('=')
    a, b = span.split('-')
    s = [d for d in stats if a <= clk(d['t_us']) <= b]
    f, l = s[0], s[-1]
    sent = l['tx_datagrams'] - f['tx_datagrams']
    lost = l['lost_packets'] - f['lost_packets']
    pkts = [d['cwnd'] / d['mtu'] for d in s]
    print(f"{name}: {sent} datagrams, {lost} lost ({100 * lost / max(sent, 1):.1f}%), "
          f"{l['congestion_events'] - f['congestion_events']} congestion events, "
          f"cwnd median {st.median(pkts):.1f} pkts, at the 2-packet minimum "
          f"{100 * sum(p <= 2 for p in pkts) / len(pkts):.0f}% of samples, "
          f"{(l['conn_tx_bytes'] - f['conn_tx_bytes']) / 1e3:.0f} KB sent in {len(s)} s")
