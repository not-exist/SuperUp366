# app.u3a / .u3enc 解密：硬结论（baseline 采集）

## 1. 找到了 .u3enc 的 AES 实现（源码级证据）

up366.exe 偏移 128158192 处的内嵌 JS（Electron `wrapFsWithAsar` 内部）：

```js
const decFile = t => {
  const e = Buffer.from("QJBNiBmV55PDrewyne3GsA==", "base64");      // 16 字节密钥
  N = N || require("crypto");
  const s = Buffer.from(t.buffer, t.byteOffset, 16);                  // 前 16 字节 = IV
  const n = N.createDecipheriv("aes-128-cbc", e, s);
  return Buffer.concat([n.update(Buffer.from(t.buffer, t.byteOffset+16, t.length-16)), n.final()])
}
```

- 密钥 hex：`40904d881995e793c3adec329dedc6b0`
- 布局：`[16B IV][AES-128-CBC(rest)]`，PKCS7
- 加密范围（JS 里的白名单）：`.xml .js .html .htm .css` → 文件名追加 `.u3enc`

## 2. 实测验证

- 镜像内 43 个 `*.u3enc`：42 个 directly 解出可读文本；第 43 个
  `resources/res/static/login/login.bdb7e7c2.js.u3enc` 也可读（`/*! @version v1.1.2` 开头，
  只是中文注释让 printable=0.847），即 43/43 全部成功。
- 样例：`dialog.html.u3enc` → `<html><head><title>天学网客户端</title>...`
  `plugins/ases/index.js.u3enc` → `"use strict";...`

## 3. app.u3a 与 ASAR 的关系（推翻"自定义二进制"）

- up366.exe 内字符串显示它**就是** Electron 的 asar，只是把 "asar" 字样改成了 "u3a"：
  - 绑定名 `electron_common_asar`，类 `Archive`，`new c.Archive(t)`
  - 源文件名仍是 `electron/shell/common/asar/archive.cc` / `asar_util.cc`
  - 报错串 `Failed to parse header`、`Integrity check failed for u3a archive`
- 但 app.u3a 的头部 **不是**明文 Pickle/JSON：整文件 16 字节对齐全扫，
  解密后以 `{"files"` 开头的块为 0，`{"files"`/`"size"`/`"offset"` 明文出现次数 0。
  → 头部被单独加密/改造，需要原生层分析。

## 4. app.u3a 主体是"明文文件 + AES 条目"混排（解释 chi²=18616）

- 4096..12288：明文 package.json（up366 6.11.0，含依赖表）+ HTML 页 + `GIF89a`（@9061）。
- 16384 之后按 4096 分块统计，在"高熵近均匀(R)"与"结构化(B)"之间反复切换 —— 混合样本，
  所以整体卡方远超均匀、0x00/0x80/0x40 偏高，**不是**单一变换（位平移 1..7 全部试过，无解）。
- 同一个 AES 密钥在 app.u3a 内直接可用：
  - 偏移 5328..5344 为 IV，5344 起为密文 → 解出
    `"use strict";"use strict";\nvar k=Object.create;var c=Object.defineProperty...`，
    末尾是 webpack 的 `0&&(module.exports={Devic...`，条目长约 2.5KB。
- 即：app.u3a 里的 `.js/.html/.css/.xml` 条目同样是 `[IV][CBC]` 内联存放，
  **不需要头部索引也能扫描 16 字节对齐位置逐个抠出**。

## 5. 尚未解决

- asar 头部（文件名/偏移/size 索引）的加密方式未知；已排除同密钥 CBC/ECB 多种 IV 组合、
  662 个 exe 内 base64 常量候选、位平移、zlib/brotli/zip/asar 明文。
