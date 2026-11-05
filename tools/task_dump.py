# -*- coding: utf-8 -*-
import json, io, os, re, urllib.parse, subprocess, tempfile

BASE = r'C:\Up366StudentFiles\resources\2115269800157515843'
TOOL = r'H:/project/up366/tools/u3enc.js'

def parse_paper(path):
    s = io.open(path, encoding='utf-8').read()
    out = []
    for m in re.finditer(r'<element id="([^"]+)" type="([^"]+)">(.*?)</element>', s, re.S):
        eid, etype, body = m.groups()
        def tag(t):
            mm = re.search(r'<%s>(.*?)</%s>' % (t, t), body, re.S)
            return mm.group(1).strip() if mm else ''
        dec = urllib.parse.unquote_plus(tag('attachment'))
        pairs = re.findall(r'<content>\s*<!\[CDATA\[(.*?)\]\]>\s*</content>\s*<content_cn>\s*<!\[CDATA\[(.*?)\]\]>', dec, re.S)
        nets = re.findall(r'netfile="([^"]+)"', dec)
        out.append(dict(id=eid, qtype=tag('question_type'), score=tag('question_score'),
                        dur=tag('duration'), stype=tag('qlib_qst_type_name'),
                        pairs=pairs, nets=nets))
    return out

src = os.path.join(BASE, 'paper.xml')
if not os.path.exists(src):
    src = os.path.join(tempfile.gettempdir(), 'u4paper.xml')
    subprocess.run(['node', TOOL, 'decrypt', os.path.join(BASE, 'paper.xml.u3enc'), src], check=True, capture_output=True)

paper = parse_paper(src)
for i, p in enumerate(paper, 1):
    print('Q%d|%s|%s|%s|%s|%d|%d' % (i, p['id'], p['qtype'], p['score'], p['dur'], len(p['pairs']), len(p['nets'])))
    for j, (en, cn) in enumerate(p['pairs'], 1):
        net = p['nets'][j-1] if j-1 < len(p['nets']) else ''
        print('S%d|%d|%s|%s|%s' % (i, j, net, en.strip(), cn.strip()))
