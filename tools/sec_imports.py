import struct, sys, re
CRYPTO = re.compile(r'^(Crypt|BCrypt|NCrypt|Ssl|SHA|MD5|HMAC|RtlEncrypt|BCryptGenRandom)', re.I)
NET = re.compile(r'^(socket|connect|send|recv|WSA|WinHttp|InternetOpen|InternetConnect|HttpOpenRequest|HttpSendRequest)', re.I)
def imports(path):
    d=open(path,'rb').read()
    pe=struct.unpack_from('<I',d,0x3c)[0]
    nsec=struct.unpack_from('<H',d,pe+6)[0]
    optsz=struct.unpack_from('<H',d,pe+20)[0]
    opt=pe+24
    magic=struct.unpack_from('<H',d,opt)[0]
    nrva_off = opt+92 if magic==0x10b else opt+108
    ddoff=nrva_off+4
    secs=[]
    so=pe+24+optsz
    for i in range(nsec):
        o=so+40*i
        vs,va,rs,ra=struct.unpack_from('<IIII',d,o+8)
        secs.append((va,max(vs,rs),ra,rs))
    def r2o(rva):
        for va,vs,ra,rs in secs:
            if va<=rva<va+vs: return ra+(rva-va)
        return None
    out={}
    for idx,label in ((1,'import'),(13,'delay')):
        rva,sz=struct.unpack_from('<II', d, ddoff+8*idx)
        if not rva or not sz: continue
        o=r2o(rva)
        if o is None: continue
        i=0
        while True:
            ent=o+20*i
            if ent+20>len(d): break
            ilt,ts,fc,nm,iat=struct.unpack_from('<IIIII',d,ent)
            if nm==0 and ilt==0: break
            no=r2o(nm)
            dll=d[no:d.index(b'\0',no)].decode('latin1') if no else '?'
            fns=[]
            t=ilt or iat; to=r2o(t)
            if to:
                j=0
                while True:
                    v,=struct.unpack_from('<I',d,to+4*j)
                    if v==0: break
                    if v & 0x80000000: fns.append('Ord#%d'%(v&0xffff))
                    else:
                        fo=r2o(v); 
                        fns.append(d[fo+2:d.index(b'\0',fo+2)].decode('latin1') if fo else '?')
                    j+=1
                    if j>5000: break
            out.setdefault(label,[]).append((dll,fns))
            i+=1
            if i>300: break
    return out
for p in sys.argv[1:]:
    print("#### %s" % p)
    try:
        im=imports(p)
    except Exception as e:
        print("  ERR", e); continue
    for label,dl in im.items():
        for dll,fns in dl:
            print("  [%s] %s (%d)" % (label, dll, len(fns)))
            c=[f for f in fns if CRYPTO.match(f)]
            n=[f for f in fns if NET.match(f)]
            if c: print("      CRYPTO:", c[:20])
            if n: print("      NET:", n[:25])
