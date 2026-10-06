import sys
R0=sys.argv[1]; sys.argv=sys.argv[1:]; exec(open(R0).read().split("print(f\"{'window'")[0])
R=146000
print('window | model floor winnable | synthHJ(%win) synth+poll1(%win) | B=40KB/s: base comp synth synth+comp | relayed: base synthH synthHJ synth+poll1+comp floorR')
for name, a, b in W:
    ev = prep(a, b)
    base = sim(ev, a, b)
    floor = sim(ev, a, b, rtt=1, synth=('H','J'), c_local=0, pJ=0, pH=0, B=1e12)
    win = base - floor
    s = sim(ev, a, b, synth=('H','J')); sp = sim(ev, a, b, synth=('H','J'), c_local=1000, pJ=1000, pH=1000)
    lb = [sim(ev, a, b, B=40e3), sim(ev, a, b, B=40e3, comp=0.1), sim(ev, a, b, B=40e3, synth=('H','J')), sim(ev, a, b, B=40e3, synth=('H','J'), comp=0.1)]
    rb = sim(ev, a, b, rtt=R); rh = sim(ev, a, b, rtt=R, synth=('H',)); rhj = sim(ev, a, b, rtt=R, synth=('H','J'))
    rall = sim(ev, a, b, rtt=R, synth=('H','J'), c_local=1000, pJ=1000, pH=1000, comp=0.1)
    print(f'{name:11} | {base:5.1f} {floor:5.1f} {win:5.1f} | {s:5.1f} ({100*(base-s)/win:3.0f}%) {sp:5.1f} ({100*(base-sp)/win:3.0f}%) | '
          + ' '.join(f'{x:5.1f}' for x in lb) + f' | {rb:5.1f} {rh:5.1f} {rhj:5.1f} {rall:5.1f} {floor:5.1f} (winR {rb-floor:4.1f}, synth+poll+comp wins {100*(rb-rall)/(rb-floor):3.0f}%)')
# resends per window: total resend bytes, and those sent after the ack had reached the Helper
print()
for name, a, b in W:
    H, J = cycles(a, b)
    rb_ = sum((len(h['copies'])-1)*h['size'] for h in H); n = sum(len(h['copies'])-1 for h in H)
    late = [(c, h) for h in H for c in h['copies'][1:] if c['t_us'] > h['ta']]
    print(f'{name}: host resends {n} ({rb_} B); after the ack reached the Helper: {len(late)} ({sum(h["size"] for c,h in late)} B); joiner resends {sum(len(j["copies"])-1 for j in J)}')
