import sys,re
p=sys.argv[1]; kw=sys.argv[2]; w=int(sys.argv[3])
d=open(p,'rb').read()
low=d.lower()
for m in list(re.finditer(re.escape(kw.lower().encode()), low))[:5]:
    o=m.start()
    frag=d[max(0,o-w):o+w]
    print("@%d: %s" % (o, re.sub(rb'[^\x20-\x7e]', b'.', frag).decode('latin1')))
    print()
