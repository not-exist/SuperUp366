import struct, sys, math, collections
p = sys.argv[1]
d = open(p,'rb').read()
pe = struct.unpack_from('<I', d, 0x3c)[0]
assert d[pe:pe+4]==b'PE\0\0'
nsec = struct.unpack_from('<H', d, pe+6)[0]
optsz = struct.unpack_from('<H', d, pe+20)[0]
magic = struct.unpack_from('<H', d, pe+24)[0]
opt = pe+24
if magic==0x10b:
    imgsz=struct.unpack_from('<I',d,opt+56)[0]; nrva_off=opt+92
else:
    imgsz=struct.unpack_from('<I',d,opt+56)[0]; nrva_off=opt+108
nrva = struct.unpack_from('<I', d, nrva_off)[0]
ddoff = nrva_off+4
def dd(i):
    return struct.unpack_from('<II', d, ddoff+8*i)
print("Machine=%04x NumSec=%d OptMagic=%x ImageSize=%d NumRVA=%d Subsystem=%d DllCharacteristics=%04x" % (
  struct.unpack_from('<H',d,pe+4)[0], nsec, magic, imgsz, nrva,
  struct.unpack_from('<H',d,opt+68)[0], struct.unpack_from('<H',d,opt+70)[0]))
names=["Export","Import","Resource","Exception","Security","Reloc","Debug","Arch","GlobalPtr","TLS","LoadConfig","BoundImport","IAT","DelayImport","CLR","Reserved"]
for i in range(min(nrva,16)):
    rva,sz = dd(i)
    print("  DD[%d] %-12s rva=%#x size=%d" % (i, names[i], rva, sz))
sec_off = pe+24+optsz
def ent(b):
    if not b: return 0
    c=collections.Counter(b); n=len(b)
    return -sum(v/n*math.log2(v/n) for v in c.values())
print("Sections:")
for i in range(nsec):
    o = sec_off+40*i
    name = d[o:o+8].rstrip(b'\0').decode('latin1')
    vs,va,rs,ra = struct.unpack_from('<IIII', d, o+8)
    ch = struct.unpack_from('<I', d, o+36)[0]
    body = d[ra:ra+rs]
    print("  %-8s VA=%#08x VSize=%-9d RawOff=%-9d RawSize=%-9d Entropy=%.2f Chars=%#x" % (name,va,vs,ra,rs,ent(body),ch))
print("FileSize=%d" % len(d))
