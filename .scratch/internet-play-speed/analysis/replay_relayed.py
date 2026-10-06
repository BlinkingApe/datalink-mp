import sys
R0=sys.argv[1]; sys.argv=sys.argv[1:]; exec(open(R0).read().split("print(f\"{'window'")[0])
R=146000
for name, a, b in W:
    ev = prep(a, b)
    rb = sim(ev, a, b, rtt=R); fl = sim(ev, a, b, rtt=1, synth=('H','J'), c_local=0, pJ=0, pH=0, B=1e12)
    r = dict(poll1=sim(ev, a, b, rtt=R, pJ=1000, pH=1000, ack_excess=2000), comp=sim(ev, a, b, rtt=R, comp=0.1),
             synth_sendsig=sim(ev, a, b, rtt=R, synth=('H','J'), c_local=500),
             synthHJ_B40=sim(ev, a, b, rtt=R, synth=('H','J'), B=40e3), synthHJ_B40_comp=sim(ev, a, b, rtt=R, synth=('H','J'), B=40e3, comp=0.1))
    d = dict(synth_sendsig=sim(ev, a, b, synth=('H','J'), c_local=500))
    print(f'{name:11} relayed base {rb:5.1f} winnable {rb-fl:4.1f} | ' + ' '.join(f'{k} {v:5.1f}' for k, v in r.items()) + ' | direct ' + ' '.join(f'{k} {v:5.1f}' for k, v in d.items()))
