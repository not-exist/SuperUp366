import sys, re
p = sys.argv[1]
kws = [k.lower() for k in sys.argv[2].split(",")]
show = int(sys.argv[3]) if len(sys.argv)>3 else 3
data = open(p,'rb').read()
low = data.lower()
def clean(b):
    return re.sub(r'[^\x20-\x7e]', '.', b.decode('latin1'))
for kw in kws:
    a = kw.encode('ascii'); u = kw.encode('utf-16-le')
    ha = [m.start() for m in re.finditer(re.escape(a), low)]
    hu = [m.start() for m in re.finditer(re.escape(u), low)]
    print("== %s ascii=%d utf16=%d" % (kw, len(ha), len(hu)))
    for o in ha[:show]:
        print("   A @%d  ...%s..." % (o, clean(data[max(0,o-50):o+70])))
    for o in hu[:show]:
        print("   U @%d  ...%s..." % (o, clean(data[max(0,o-50):o+120])))
