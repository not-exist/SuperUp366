import struct, sys, re
p = sys.argv[1]
d = open(p,'rb').read()
pe = struct.unpack_from('<I', d, 0x3c)[0]
nsec = struct.unpack_from('<H', d, pe+6)[0]
optsz = struct.unpack_from('<H', d, pe+20)[0]
opt = pe+24
nrva_off = opt+92
ddoff = nrva_off+4
def dd(i): return struct.unpack_from('<II', d, ddoff+8*i)
def rva2off(rva, secs):
    for name,va,vs,ra,rs in secs:
        if va <= rva < va+max(vs,rs): return ra + (rva-va)
    return None
sec_off = pe+24+optsz
secs=[]
for i in range(nsec):
    o=sec_off+40*i
    name=d[o:o+8].rstrip(b'\0').decode('latin1')
    vs,va,rs,ra=struct.unpack_from('<IIII', d, o+8)
    secs.append((name,va,vs,ra,rs))
rva,sz = dd(6)
off = rva2off(rva, secs)
for i in range(sz//28):
    b = off+28*i
    charset,=struct.unpack_from('<I',d,b+4)
    ts,maj,mn,typ,dsz,daddr,dptr = struct.unpack_from('<IHHIIII', d, b+8)
    s = d[dptr:dptr+dsz]
    print("DEBUG[%d] type=%d size=%d ts=%d maj=%d min=%d payload=%r" % (i,typ,dsz,ts,maj,mn,s[:260]))
# TLS
rva,sz = dd(9)
if rva:
    o=rva2off(rva,secs)
    start,end,idx,cb = struct.unpack_from('<IIII', d, o)
    print("TLS: Start=%#x End=%#x IndexAddr=%#x CallbacksVA=%#x" % (start,end,idx,cb))
    co = rva2off(cb - struct.unpack_from('<I', d, opt+28)[0], secs) if cb else None
    print("  callbacks_off=%r" % co)
    if co is not None:
        cbs=[]
        while True:
            v,=struct.unpack_from('<I', d, co+4*len(cbs))
            if v==0: break
            cbs.append(hex(v))
            if len(cbs)>8: break
        print("  callbacks:", cbs)
print("Checksum/Subsystem; EntryPoint=%#x" % struct.unpack_from('<I', d, opt+16)[0])
