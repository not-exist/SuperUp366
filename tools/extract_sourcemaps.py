#!/usr/bin/env python3
"""Static extraction of sourcesContent from the up366 xotFullScreen source maps.

Read-only w.r.t. the inputs: the .map files are parsed as JSON and the embedded
sourcesContent strings are written out verbatim. No code inside a map is ever
evaluated.

Usage:
    python tools/extract_sourcemaps.py
"""

import io
import json
import os
import re
import sys

BASE = r"C:/Program Files (x86)/up366/client/resources/res/xotFullScreen"
MAPS = [
    "js/app.1ae30858.js.map",
    "js/chunk-vendors.891aa818.js.map",
    "css/app.8252afd4.css.map",
]
OUT = r"H:/project/up366/extract/xotFullScreen-src"

WEBPACK_RE = re.compile(r"^webpack:/+")
QUERY_RE = re.compile(r"\?[0-9a-zA-Z_\-]{4}$")
DRIVE_RE = re.compile(r"[A-Za-z]:/")
BAD_CHARS_RE = re.compile(r'[<>:"|?*\x00-\x1f]')


def strip_prefix(src):
    """Drop the webpack:// scheme, a loader query and surrounding whitespace."""
    p = src.strip()
    p = WEBPACK_RE.sub("", p)
    p = p.strip()
    p = QUERY_RE.sub("", p)
    return p


def last_drive_index(p):
    """Index of the last 'X:/' occurrence, or -1."""
    idx = -1
    for m in DRIVE_RE.finditer(p):
        idx = m.start()
    return idx


def sanitize(component):
    component = BAD_CHARS_RE.sub("_", component)
    component = component.rstrip(". ")
    return component or "_"


def plan_path(src):
    """Map an original source name to a relative output path (posix style).

    Returns (relpath, kind) where kind is one of: relative, node_modules,
    absolute. Returns (None, 'escape') when the path would leave the root.
    """
    p = strip_prefix(src)
    if not p:
        return None, "empty"
    p = p.replace("\\", "/")

    if DRIVE_RE.search(p):
        # Absolute build-machine path. Some entries prepend the webpack source
        # root (e.g. 'src/scss/D:/work/...' and even a duplicated copy of the
        # root), so keep only the final drive-qualified portion.
        p = p[last_drive_index(p):]
        parts = [c for c in p.split("/") if c]
        drive = parts[0][0].upper()  # 'D:' -> 'D'
        parts = [drive] + parts[1:]
        parts = [sanitize(c) for c in parts]
        return "/".join(["_absolute"] + parts), "absolute"

    # Relative / bare path: normalise without ever touching the filesystem.
    parts = []
    for comp in p.split("/"):
        if comp in ("", "."):
            continue
        if comp == "..":
            if not parts:
                return None, "escape"
            parts.pop()
            continue
        parts.append(sanitize(comp))
    if not parts:
        return None, "empty"

    if "node_modules" in parts:
        cut = parts.index("node_modules")
        return "/".join(["node_modules"] + parts[cut + 1:]), "node_modules"
    return "/".join(parts), "relative"


def unique_path(rel, used):
    """Never overwrite: append -1, -2, ... before the extension."""
    key = rel.lower()
    if key not in used:
        used.add(key)
        return rel
    root, ext = os.path.splitext(rel)
    n = 1
    while True:
        cand = "%s-%d%s" % (root, n, ext)
        if cand.lower() not in used:
            used.add(cand.lower())
            return cand
        n += 1


def main():
    out_root = os.path.abspath(OUT)
    os.makedirs(out_root, exist_ok=True)

    used = set()
    index_rows = []
    reports = []
    total_bytes = 0

    for name in MAPS:
        path = os.path.join(BASE, name)
        with io.open(path, "r", encoding="utf-8") as fh:
            doc = json.load(fh)

        sources = doc.get("sources") or []
        contents = doc.get("sourcesContent") or []
        written = skipped_empty = skipped_escape = 0
        map_bytes = 0
        kinds = {}

        for i, src in enumerate(sources):
            content = contents[i] if i < len(contents) else None
            if not content:
                skipped_empty += 1
                continue
            rel, kind = plan_path(src)
            if rel is None:
                skipped_escape += 1
                continue
            rel = unique_path(rel, used)

            dest = os.path.abspath(os.path.join(out_root, rel.replace("/", os.sep)))
            if not dest.startswith(out_root + os.sep):
                skipped_escape += 1
                continue

            try:
                data = content.encode("utf-8")
            except UnicodeEncodeError:
                data = content.encode("utf-8", "surrogatepass")

            parent = os.path.dirname(dest)
            if parent:
                os.makedirs(parent, exist_ok=True)
            with open(dest, "wb") as fh:
                fh.write(data)

            written += 1
            map_bytes += len(data)
            total_bytes += len(data)
            kinds[kind] = kinds.get(kind, 0) + 1
            index_rows.append((rel, len(data)))

        reports.append({
            "map": name,
            "file": doc.get("file"),
            "sources": len(sources),
            "written": written,
            "skipped_empty": skipped_empty,
            "skipped_escape": skipped_escape,
            "bytes": map_bytes,
            "kinds": kinds,
        })

    index_rows.sort(key=lambda r: r[0])
    with io.open(os.path.join(out_root, "_files.txt"), "w", encoding="utf-8", newline="") as fh:
        for rel, size in index_rows:
            fh.write("%s\t%d\n" % (rel, size))

    for r in reports:
        print("%-32s file=%-28s sources=%-4d written=%-4d empty=%-3d escape=%-2d bytes=%d"
              % (r["map"], r["file"], r["sources"], r["written"],
                 r["skipped_empty"], r["skipped_escape"], r["bytes"]))
        print("    kinds:", r["kinds"])
    print("TOTAL files=%d bytes=%d" % (len(index_rows), total_bytes))
    return reports, index_rows


if __name__ == "__main__":
    main()
