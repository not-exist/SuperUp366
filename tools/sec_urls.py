import sys,re
p=sys.argv[1]
d=open(p,'rb').read()
pats=[rb'https?://[\x21-\x7e]{4,120}', rb'/(client|api|student)/[A-Za-z0-9_/\-]{2,60}']
for pat in pats:
    seen=[]
    for m in re.finditer(pat,d):
        s=m.group().decode('latin1')
        if s not in seen: seen.append(s)
    print("PATTERN",pat,"count",len(seen))
    for s in seen[:40]: print("  ",s)
