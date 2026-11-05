# xotFullScreen — source map reconstruction

Static parse only. The `.map` files were read as JSON and the embedded
`sourcesContent` strings were written out byte-for-byte. Nothing from a map was
executed; no network access; inputs under `C:\Program Files (x86)` were read-only.

- Inputs: `client/resources/res/xotFullScreen/{js,css}/*.map`
- Output: `H:/project/up366/extract/xotFullScreen-src/`
- Script: `H:/project/up366/tools/extract_sourcemaps.py`
- Index: `H:/project/up366/extract/xotFullScreen-src/_files.txt` (`<relpath>\t<bytes>`)

## Per-map statistics

| map | `file` field | sources | written | skipped (empty) | skipped (escape) | bytes |
|---|---|---|---|---|---|---|
| `js/app.1ae30858.js.map` | `js/app.1ae30858.js` | 17 | 17 | 0 | 0 | 1,787,525 |
| `js/chunk-vendors.891aa818.js.map` | `js/chunk-vendors.891aa818.js` | 152 | 152 | 0 | 0 | 884,127 |
| `css/app.8252afd4.css.map` | `app.8252afd4.css` | 4 | 4 | 0 | 0 | 582,095 |
| **total** | | **173** | **173** | **0** | **0** | **3,253,747** |

All three maps are version 3 with a complete `sourcesContent` array — 173 of 173
entries had content, so nothing was skipped. `sourceRoot` is `""` for the two JS
maps and absent for the CSS map.

Verification performed: every one of the 173 written files is byte-identical to
its `sourcesContent` value (multiset comparison, 0 unmatched, 0 leftover blobs),
and every size in `_files.txt` matches the file on disk. Line endings were left
untouched — 11 files still contain CRLF, 157 are LF-only.

## Reconstructed build machine root

`D:/work/fullscreen-html` — inferred from the absolute paths embedded in
`css/app.8252afd4.css.map`, and consistent with the webpack source root of the two
JS maps (their `./src/...` and `./node_modules/...` specifiers resolve under it).

```
D:/work/fullscreen-html/              <- webpack context root
├── node_modules/                     <- ./node_modules/* in chunk-vendors
└── src/
    ├── main-offline.js
    ├── components/XotPfs.vue
    ├── view/offline/offline-pfs.vue
    ├── utils/{flexible.js, flipToXot.js}
    ├── scss/reset.scss               <- ABS conf: D:/work/fullscreen-html/src/scss/reset.scss
    └── lib/
        ├── U3CompoLib-1.0.0/
        │   ├── U3CompoLib.js
        │   └── css/theme.css         <- ABS conf: D:/work/fullscreen-html/src/lib/U3CompoLib-1.0.0/css/theme.css
        └── xot/xot-pfs/build/
            ├── preview-fullscreen.js
            └── preview.fullscreen.css <- ABS conf: D:/work/fullscreen-html/src/lib/xot/xot-pfs/build/preview.fullscreen.css
```

Directories confirmed by absolute path: `src/scss/`, `src/lib/U3CompoLib-1.0.0/css/`,
`src/lib/xot/xot-pfs/build/`. The rest of the tree comes from relative webpack
specifiers (`./src/components/...`, `./src/utils/...`, `./node_modules/...`), which
only pin the layout relative to the same root.

## Which extracted file is the real source

The maps contain several entries per `.vue` file — vue-loader emits shims for the
template, script and style blocks, and they all normalise to the same filename.
Collisions were resolved first-claim-wins with `-1`/`-2`/`...` suffixes, and the
*first* claim is usually a compiled artifact, **not** the original SFC:

| what you want | file to open |
|---|---|
| `XotPfs.vue` original SFC (`<template>`+`<script>`+`<style>`, 3,234 B) | `src/components/XotPfs-1.vue` |
| `offline-pfs.vue` original SFC (1,573 B) | `src/view/offline/offline-pfs-1.vue` |
| real `reset.scss` (1,543 B) | `_absolute/D/work/fullscreen-html/src/scss/reset.scss` |
| real theme CSS (75,461 B) | `_absolute/D/work/fullscreen-html/src/lib/U3CompoLib-1.0.0/css/theme.css` |
| real player CSS (124,488 B) | `_absolute/D/work/fullscreen-html/src/lib/xot/xot-pfs/build/preview.fullscreen.css` |

Decoys, kept but not source: `src/components/XotPfs.vue` (167 B, compiled
`render`/`staticRenderFns`), `XotPfs-2.vue` (658 B, babel `script` shim),
`XotPfs-3.vue` (565 B, component normalizer), `XotPfs-4.vue` (39 B,
`// extracted by mini-css-extract-plugin`), `XotPfs-5.vue` (555 B, style shim);
the same pattern repeats for `offline-pfs{,-2,-3}.vue` and `src/scss/reset.scss` (39 B,
also a mini-css-extract stub).

## Larger observations

- `src/lib/xot/xot-pfs/build/preview-fullscreen.js` is 1,623,387 B — the player is
  shipped as a single pre-bundled file inside `src/lib`. It is 50% of the whole
  app-map payload and dominated it. `src/lib/U3CompoLib-1.0.0/U3CompoLib.js`
  (141,010 B) is the same story.
- `app.8252afd4.css` (380,603 B) is the compiled stylesheet itself, present in its
  own map as a source entry named `app.8252afd4.css` with no `webpack://` prefix.
- CSS map source names carry a duplicated prefix, e.g.
  `webpack:///src/scss/D:/work/fullscreen-html/src/scss/D:/work/fullscreen-html/src/scss/reset.scss`.
  That is a css-loader `sourceRoot` concatenated in front of an already-absolute
  path. The extractor keeps only the final drive-qualified portion, so no
  duplicated directories leak into the output tree.

## Path mapping rules applied

1. Strip the `webpack://` scheme and any trailing loader query of the form
   `?0e8d` (4+ alnum characters).
2. Absolute (drive-qualified) paths → `_absolute/<DRIVE>/...` with the original
   hierarchy preserved and the drive colon dropped (illegal on Windows), so
   `D:/work/fullscreen-html/src/scss/reset.scss` becomes
   `_absolute/D/work/fullscreen-html/src/scss/reset.scss`.
3. Paths containing a `node_modules` component → rooted at `node_modules/`.
4. `./` and `../` segments normalised lexically; a `..` that would climb above the
   output root is rejected and counted as an escape (0 occurred).
5. Illegal Windows characters sanitised to `_`; collisions suffixed `-1`, `-2`, ...
   Nothing is ever overwritten. All output is UTF-8 with original line separators.
