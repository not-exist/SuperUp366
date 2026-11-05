import sys, re
p=sys.argv[1]; off=int(sys.argv[2],0); ln=int(sys.argv[3],0)
d=open(p,'rb').read()[off:off+ln]
for m in re.finditer(rb'[\x20-\x7e]{5,}', d):
    print("%#x %s" % (off+m.start(), m.group().decode('latin1')))
