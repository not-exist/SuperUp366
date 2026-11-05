# -*- coding: utf-8 -*-
import io, os

W = r'H:/project/up366'
NL = chr(10)
rows = io.open(os.path.join(W, 'work/task_unit4/task.txt'), encoding='utf-8').read().splitlines()

qrows = [r.split('|') for r in rows if r.startswith('Q')]
srows = [r.split('|') for r in rows if r.startswith('S')]

qtab = ['| # | elementId | 题型 | 分值 | 时长(s) | 句数 | 参考音频 |', '|---|---|---|---|---|---|---|']
for f in qrows:
    qtab.append('| %s | %s | %s 口语跟读 | %s | %s | %s | %s 个 .net |' % (f[0][1:], f[1], f[2], f[3], f[4], f[5], f[6]))

blocks = []
for f in qrows:
    num = f[0][1:]
    body = ['| # | 参考音频 | 原文 | 译文 |', '|---|---|---|---|']
    for g in srows:
        if g[0] != 'S' + num:
            continue
        body.append('| %s | %s | %s | %s |' % (g[1], g[2], g[3].replace('|', '/'), g[4].replace('|', '/')))
    blocks.append('### 第 %s 题（%s）' % (num, f[1]) + NL + NL + NL.join(body))

doc = io.open(os.path.join(W, 'docs/task-unit4-template.md'), encoding='utf-8').read()
doc = doc.replace('{{QTABLE}}', NL.join(qtab))
doc = doc.replace('{{STABLE}}', (NL + NL).join(blocks))
io.open(os.path.join(W, 'recon/task-unit4-readaloud.md'), 'w', encoding='utf-8').write(doc)
print('ok', len(doc), 'lines', doc.count(NL) + 1)
