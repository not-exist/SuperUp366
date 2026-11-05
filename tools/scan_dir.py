import os, sys, re
root = sys.argv[1]
kws = [k.lower() for k in sys.argv[2].split(",")]
res = {}
for dp, dn, fn in os.walk(root):
    for f in fn:
        p = os.path.join(dp, f)
        try:
            st = os.stat(p)
            if st.st_size > 400*1024*1024: continue
            d = open(p,'rb').read()
        except Exception: continue
        low = d.lower()
        hits = {}
        for kw in kws:
            a = len(re.findall(re.escape(kw.encode('ascii')), low))
            u = len(re.findall(re.escape(kw.encode('utf-16-le')), low))
            if a or u: hits[kw] = (a,u)
        if hits: res[os.path.relpath(p, root)] = hits
for k,v in sorted(res.items()):
    print(k, "->", v)
print("files_with_hits=", len(res))
