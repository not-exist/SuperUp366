import sys, re
p=sys.argv[1]; kw=sys.argv[2]; w=int(sys.argv[3]) if len(sys.argv)>3 else 300
lim=int(sys.argv[4]) if len(sys.argv)>4 else 5
d=open(p,'rb').read().decode('utf-8','replace')
idxs=[m.start() for m in re.finditer(re.escape(kw.lower()), d.lower())]
print("hits=%d len=%d" % (len(idxs), len(d)))
for i in idxs[:lim]:
    print("---@%d---" % i)
    print(d[max(0,i-w):i+w])
