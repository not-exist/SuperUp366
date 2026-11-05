#!/usr/bin/env python3
r"""Locate every trace of a feature name inside an Electron install tree.

Three encodings are covered because the same Chinese label can be stored as:
  utf8    raw UTF-8 bytes           (html / json / uncompressed js)
  utf16   UTF-16LE                  (Windows resources, some .node)
  escape  \uXXXX escapes            (minified js - plain grep misses these)

Usage:
  python tools/find_feature.py "单词PK"
  python tools/find_feature.py "单词PK" --root "C:\Program Files (x86)\up366\client"
"""
import argparse
import os
import re

SKIP_EXT = {'.png', '.jpg', '.jpeg', '.gif', '.ico', '.icns', '.woff', '.woff2',
            '.ttf', '.wav', '.mp3', '.model', '.mdl', '.fst', '.onnx', '.pak'}


def esc_variants(text):
    r'''Turn Chinese into the \uXXXX form used inside minified js.'''
    low = ''.join('\\u%04x' % ord(c) if ord(c) > 127 else c for c in text)
    up = ''.join('\\u%04X' % ord(c) if ord(c) > 127 else c for c in text)
    return {low, up}


def scan(root, needle, max_bytes):
    hits = []
    raw = needle.encode('utf-8')
    wide = needle.encode('utf-16-le')
    escapes = esc_variants(needle)
    for dp, _, files in os.walk(root):
        for fn in files:
            if os.path.splitext(fn)[1].lower() in SKIP_EXT:
                continue
            p = os.path.join(dp, fn)
            try:
                if os.path.getsize(p) > max_bytes:
                    continue
                blob = open(p, 'rb').read()
            except Exception:
                continue
            rel = os.path.relpath(p, root)
            for label, pat in (('utf8', raw), ('utf16', wide)):
                start = 0
                while True:
                    i = blob.find(pat, start)
                    if i < 0:
                        break
                    hits.append((rel, label, i))
                    start = i + 1
                    if len(hits) > 2000:
                        return hits
            if any(ord(c) > 127 for c in needle):
                text = blob.decode('utf-8', 'replace')
                for enc in escapes:
                    for m in re.finditer(re.escape(enc), text):
                        hits.append((rel, 'escape', m.start()))
                        if len(hits) > 2000:
                            return hits
    return hits


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('needle')
    ap.add_argument('--root', default=r'C:\Program Files (x86)\up366\client')
    ap.add_argument('--max-bytes', type=int, default=64 * 1024 * 1024)
    ap.add_argument('--limit', type=int, default=40)
    a = ap.parse_args()

    hits = scan(a.root, a.needle, a.max_bytes)
    print('needle=%r  hits=%d' % (a.needle, len(hits)))
    seen = set()
    shown = 0
    for rel, enc, off in hits:
        if (rel, enc) in seen:
            continue
        seen.add((rel, enc))
        print('  %-62s %-7s @%d' % (rel, enc, off))
        shown += 1
        if shown >= a.limit:
            print('  ... (more)')
            break


if __name__ == '__main__':
    main()
