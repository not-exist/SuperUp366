import re, sys, os
# usage: grep_ctx.py <file> <regex> [before] [after] [limit]
path, pat = sys.argv[1], sys.argv[2]
before = int(sys.argv[3]) if len(sys.argv) > 3 else 80
after = int(sys.argv[4]) if len(sys.argv) > 4 else 160
limit = int(sys.argv[5]) if len(sys.argv) > 5 else 6
data = open(path, 'rb').read().decode('utf-8', 'replace')
n = 0
for m in re.finditer(pat, data):
    s = max(0, m.start() - before); e = min(len(data), m.end() + after)
    frag = data[s:e].replace('\n', ' ')
    print('[%d] %s' % (m.start(), frag))
    n += 1
    if n >= limit: break
print('-- matches shown: %d --' % n)
