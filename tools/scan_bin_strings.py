import sys, json, re, collections
path = sys.argv[1]
kws = [k.lower() for k in sys.argv[2].split(",")]
maxhits = int(sys.argv[3]) if len(sys.argv)>3 else 40
data = open(path,'rb').read()
low = data.lower()
out = {}
for kw in kws:
    a = kw.encode('ascii')
    u = kw.encode('utf-16-le')
    hits_ascii = [m.start() for m in re.finditer(re.escape(a), low)]
    hits_utf16 = [m.start() for m in re.finditer(re.escape(u), low)]
    def ctx(off, n):
        s = max(0, off-n); e = min(len(data), off+n)
        raw = data[s:e]
        return raw.decode('utf-8','replace').replace('\x00','.')
    rec = {"ascii_count": len(hits_ascii), "utf16_count": len(hits_utf16),
           "ascii": [{"off": o, "ctx": ctx(o,60)} for o in hits_ascii[:maxhits]],
           "utf16": [{"off": o, "ctx": ctx(o,60)} for o in hits_utf16[:maxhits]]}
    out[kw] = rec
print(json.dumps(out, ensure_ascii=False, indent=1))
