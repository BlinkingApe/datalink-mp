import json, collections, struct, sys
L=[json.loads(l) for l in open(sys.argv[1])]
msgs=[d for d in L if d['ev'] in ('out','in','loop')]
print(collections.Counter((d['ev'],d['kind'] if 'kind' in d else None,d['from'],d['to']) for d in msgs).most_common(30))
# predicted ack from data: an ack is the data message's first 12 bytes with the kind word
# rewritten to (kind & ~4) | 2. Sequence numbers repeat across recipients (setup messages go to
# player 0), so match an ack against every data message's prediction, not by seq alone.
def pred(h):
    b=bytearray(bytes.fromhex(h)[:12]); k=struct.unpack_from('<H',b,0)[0]; struct.pack_into('<H',b,0,(k&~4)|2); return b.hex()
P={pred(d['hex']) for d in msgs if d.get('kind') is not None and d['kind'] & 6 == 4}
acks=[d for d in msgs if d.get('kind') is not None and d['kind'] & 6 == 2]
mis=[d['hex'] for d in acks if d['hex'] not in P]
print('ack == first 12 bytes of a data message with kind rewritten:',len(acks)-len(mis),'mismatch',len(mis))
for e in mis[:5]: print(e)
# ack size
print(collections.Counter(d['size'] for d in msgs if d.get('kind') is not None and d['kind']&6==2))
# loop lines
print('loop:',collections.Counter((d.get('kind'),d['from'],d['to']) for d in msgs if d['ev']=='loop'))
