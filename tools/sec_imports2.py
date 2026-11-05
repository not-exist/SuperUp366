import struct, sys, re
CRYPTO = re.compile(r'(Crypt|BCrypt|NCrypt|SslEncrypt|SHA256|MD5|HMAC|RtlEncrypt)', re.I)
NET = re.compile(r'^(socket|connect|send|recv|bind|WSA|WinHttp|InternetOpen|InternetConnect|HttpOpenRequest|HttpSendRequest|getaddrinfo)', re.I)
def imports(path):
    d=open(path,'rb').read()
    pe=struct.unpack_from('<I',d,0x3c)[0]
    nsec=struct.unpack_from('<H',d,pe+6)[0]
    optsz=struct.unpack_from('<H',d,pe+20)[0]
    opt=pe+24
    magic=struct.unpack_from('<H',d,opt)[0]
    nrva_off = opt+92 if magic==0x10b else opt+108
    ddoff=nrva_off+4
    so=pe+24+optsz; secs=[]
    for i in range(nsec):
        o=so+40*i
        vs,va,rs,ra=struct.unpack_from('<IIII',d,o+8)
        secs.append((va,max(vs,rs),ra,rs))
    def r2o(rva):
        for va,vs,ra,rs in secs:
            if va<=rva<va+vs: return ra+(rva-va)
        return None
    out={}
    specs=((1,'import',20),(13,'delay',32))
    for idx,label,entsz in specs:
        rva,sz=struct.unpack_from('<II', d, ddoff+8*idx)
        if not rva or not sz: continue
        o=r2o(rva)
        if o is None: continue
        i=0; acc=[]
        while True:
            ent=o+entsz*i
            if ent+entsz>len(d): break
            if label=='import':
                ilt,ts,fc,nm,iat=struct.unpack_from('<IIIII',d,ent)
            else:
                attr,nm,mod,iat,intn,bi,ui,ts=struct.unpack_from('<IIIIIIII',d,ent)
                ilt=attr
                if nm==0: break
            if nm==0 and (ilt==0): break
            no=r2o(nm)
            if not no: break
            dll=d[no:d.index(b'\0',no)].decode('latin1')
            if not re.match(r'^[\w\-. ]+$', dll): break
            fns=[]
            t=ilt or iat; to=r2o(t) if t else None
            if to and to+4<=len(d):
                j=0
                while True:
                    v,=struct.unpack_from('<I',d,to+4*j)
                    if v==0: break
                    if v & 0x80000000: fns.append('Ord#%d'%(v&0xffff))
                    else:
                        fo=r2o(v)
                        fns.append(d[fo+2:d.index(b'\0',fo+2)].decode('latin1') if fo else '?')
                    j+=1
                    if j>8000: break
            acc.append((dll,fns)); i+=1
            if i>400: break
        if acc: out[label]=acc
    return out
for p in sys.argv[1:]:
    print("#### %s" % p.split('/')[-1])
    try: im=imports(p)
    except Exception as e: print("  ERR", e); continue
    for label,dl in im.items():
        for dll,fns in dl:
            print("  [%s] %s (%d)" % (label, dll, len(fns)))
            c=[f for f in fns if CRYPTO.search(f)]
            n=[f for f in fns if NET.match(f)]
            if c: print("      CRYPTO:", sorted(set(c))[:15])
            if n: print("      NET:", sorted(set(n))[:20])
