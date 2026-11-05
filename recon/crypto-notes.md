# up366 客户端 .u3enc / .u3a 格式静态分析

分析对象：只读目录 `C:/Program Files (x86)/up366/client`
分析方式：纯静态（字节统计 + 读二进制里的字符串常量），未运行客户端。
复现脚本：`tools/probe_u3enc.py`（Python 标准库；AES 运算交给系统 node 的内置 crypto）。
数据来源：`python tools/probe_u3enc.py` 的 JSON 输出。

---

## 0. 结论先行

| 项 | 结论 | 关键证据 |
|---|---|---|
| 算法 | **AES-128-CBC + PKCS#7** | 43/43 个文件用同一密钥成功解密，填充全部合法 |
| 密钥 | 全局唯一，硬编码在 `up366.exe` | base64 `QJBNiBmV55PDrewyne3GsA==` = hex `40904d881995e793c3adec329dedc6b0`（16 字节） |
| 每文件 IV | **有**，= 文件前 16 字节，明文存放 | 密文 = 16B IV ‖ CBC 密文 |
| ECB | **排除** | §3 |
| CTR / 流密码 | **排除** | §4 |
| 每文件独立密钥 / salt | 排除（同一密钥 43/43 成功） | §0、§5 |
| app.u3a | 同一**密码原语与密钥**，但为 asar 容器、**按条目选择性加密** | §8.1（.js/.html/.css/.xml 条目 = 16B IV + AES-128-CBC；其余条目明文；**头部未解出**） |

密钥与调用的位置（`up366.exe`，共 136,310,520 字节）：

- `128158069` — 字面量 `QJBNiBmV55PDrewyne3GsA==`
- `128158174` — `createDecipheriv("aes-128-cbc", …)`
- `128155147` / `128155267` — 扩展名正则 `/\.u3enc/i`

所在代码是被改过的 **Electron `asar/lib/fs-wrapper`**（同一段里还有 `/\.u3a/i`），核心逻辑等价于：

```js
const KEY = Buffer.from("QJBNiBmV55PDrewyne3GsA==", "base64");   // 16B
const iv  = buf.slice(0, 16);                                   // 每文件前置 IV
const dec = crypto.createDecipheriv("aes-128-cbc", KEY, iv);
plain = Buffer.concat([dec.update(buf.slice(16)), dec.final()]); // final() 做 PKCS#7 去填充
```

同段还有加密白名单 `{".xml",".js",".html",".htm",".css"}`：**只有这 5 种后缀会加 `.u3enc` 并加密**，图片 / 音频 / 字体原样存放。这解释了 `res/assets/` 下的 `.png/.jpg/.gif` 为何没有 `.u3enc`。

---

## 1. (a) 文件长度 / 16 字节对齐 / app.u3a 余数

全部 43 个 `.u3enc`：**size % 16 == 0**，无一例外。

本次取样的 13 个：

| 文件 | 密文长度 | mod 16 | 明文长度 | 填充 |
|---|---|---|---|---|
| res/index.html.u3enc | 464 | 0 | 442 | 6 |
| res/login.html.u3enc | 1280 | 0 | 1250 | 14 |
| res/network.html.u3enc | 1104 | 0 | 1078 | 10 |
| res/micTest.html.u3enc | 1104 | 0 | 1072 | 16 |
| res/dialog.html.u3enc | 800 | 0 | 777 | 7 |
| res/assets/dictation-report.a8fb8df7.js.u3enc | 832 | 0 | 811 | 5 |
| res/assets/index.2ffed8d6.js.u3enc | 1024 | 0 | 997 | 11 |
| res/assets/index.8587bc9f.css.u3enc | 672 | 0 | 643 | 13 |
| res/assets/index.b561b386.js.u3enc | 864 | 0 | 833 | 15 |
| res/assets/loading.107aab17.js.u3enc | 1152 | 0 | 1129 | 7 |
| res/assets/login.f18ed938.css.u3enc | 352 | 0 | 335 | 1 |
| res/xotFullScreen/js/app.1ae30858.js.u3enc | 1564624 | 0 | 1564601 | 7 |
| resources/plugins/ases/index.js.u3enc | 100064 | 0 | 100036 | 12 |

全量 43 个：密文合计 6,153,344；明文合计 6,152,246；总开销 **1,098 = 43×16（IV）+ 410（填充字节）**。

`app.u3a`：**13,609,735 字节，13,609,735 % 16 = 7**（850,608 个完整块 + 7 字节尾巴）。
单凭这一点即可断定 app.u3a **整文件不是** AES 块密码信封（块密码输出必然是 16 的整数倍）。

---

## 2. (b) 单文件内重复的 16 字节块

**所有 43 个 .u3enc：0 个重复块。**
（全量 384,584 个 16 字节块全部互不相同；13 个取样文件 104,646 块同样 0 重复。）

`app.u3a`：**4,255 个重复块**（占 850,608 块的 0.50%）。前 5 个（首现偏移 → 重复偏移）：

| # | 首现偏移 | 重复偏移 | 块内容 (hex) | 备注 |
|---|---|---|---|---|
| 1 | 10432 | 10512 | `31314545413441344630453932354136` | ASCII `11EEA4A4F0E925A6`，Photoshop XMP 的 GUID 片段 |
| 2 | 17600 | 26944 | `0400ff002c00000000580229010008ff` | 含 0x00 / 0xff 的结构化数据 |
| 3 | 17616 | 26960 | `00ff091c48b0a0c18308132a5cc8b0a1` | 与 #2 连续 |
| 4 | 17632 | 26976 | `c38710234a9c48b1a2c58b18336adcc8` | 与 #2 连续 |
| 5 | 17648 | 26992 | `b1a3c78f20438a1c49b2a4c9932853aa` | 与 #2 连续 |

#2–#5 是**连续 16 字节块**整体重复（17600.. 一段 ↔ 26944.. 一段），即"同一份数据在包里存了两次"，是普通归档里重复资源的特征，不是 ECB 那种散点重复。重复块中 **0 个落在前 4096 字节**，最后一个在偏移 1,226,448。

---

## 3. (c) 跨文件相同 16 字节块 —— ECB 判据

### 3.1 五个 html 的公共前缀

解密后确认明文：

- `index.html`、`login.html` 都以 `<!DOCTYPE html>\n` 开头 → **明文第 0 块完全相同**
- `network.html`、`micTest.html` 都以 `<!DOCTYPE html><` 开头 → **明文第 0 块完全相同**
- `dialog.html` 以 `<html>\n\n<head>` 开头（无 DOCTYPE）

密文前 16 / 32 / 64 / 128 字节的去重结果：

| 前缀长度 | 不同值个数 | 全部相同？ |
|---|---|---|
| 16 | **5** | 否 |
| 32 | **5** | 否 |
| 64 | **5** | 否 |
| 128 | **5** | 否 |

前 16 字节密文：

```
index.html   6197562153d731b122e1abc57ada361e
login.html   e037da88e6c32538a83b50a86c034b07
network.html ec334ad60c6026cb005ca3ee5796eaa8
micTest.html ecdaa8ee5279fa2ed0a057ffc8735ccb
dialog.html  beb9ec2b8c61945764b8c51b986dd2bf
```

**同明文 + 同密钥 → 密文不同 ⇒ 不是 ECB。** 两对"同前缀"样本（index/login、network/micTest）都给出不同的块 0，这是最直接的否证。

### 3.2 跨文件重复块统计

| 集合 | 16 字节块总数 | 跨文件重复块 | 被 ≥3 个文件共享 |
|---|---|---|---|
| 13 个取样文件 | 104,646 | **0** | 0 |
| 全部 43 个 .u3enc | 384,584 | **0** | 0 |

五个 html 两两之间的共享块数：10 个组合**全部为 0**。随机数据的期望碰撞数 ≈ n²/2¹²⁹ ≈ 1e-28，所以这是真实语义上的 0。

### 3.3 用明文重复块做定量否证（最强证据）

| 文件 | 明文 16B 块数 | 明文重复块数（占比） | 密文重复块数 |
|---|---|---|---|
| res/xotFullScreen/js/app.1ae30858.js | 97,787 | **15,159（15.5%）** | **0** |
| res/assets/index.5da9b73f.js | 62,653 | **2,446（3.9%）** | **0** |

ECB 会把明文里这 15,159 个重复块 1:1 映射成重复密文块；实测 0 个 ⇒ **ECB 明确排除**。

---

## 4. 支持 / 排除 CTR 与流密码

**排除。** 三条独立理由：

1. **长度**：43/43 个密文长度都是 16 的整数倍。流密码 / CTR 不需要块对齐，长度应等于明文长度。
2. **填充存在且分布正确**：`pt = ct - 16 - pad`，`pad` 实测覆盖 1..16 全部取值，且 node 的 `final()` 全部通过 PKCS#7 校验（43/43 无异常）。明文长度恰为 16 倍数时补满一整块（`micTest.html`：pt=1072 → pad=16），这是 PKCS#7 的确定性特征。
3. **解密可复现**：用 `iv = 前 16 字节` 的 CBC 解密，43/43 全部得到合法 UTF-8 文本（可打印率 1.000）。CTR 用同样 IV 解出来不会是这样。

长度公式在全量 43 个文件上成立：

```
ct = 16 + 16 * ceil((pt + 1) / 16)
```

---

## 5. (d) 各文件字节熵

密文熵 7.4471 → 7.9999 bit/byte，随文件增大趋近 8.0：

| 文件 | 长度 | 熵 (bit/B) |
|---|---|---|
| res/assets/login.f18ed938.css | 352 | 7.4471 |
| res/index.html | 464 | 7.5304 |
| res/assets/upcode.8489476e.css | 656 | 7.6650 |
| res/assets/index.8587bc9f.css | 672 | 7.7104 |
| res/dialog.html | 800 | 7.7316 |
| res/network.html | 1104 | 7.8545 |
| res/login.html | 1280 | 7.8573 |
| res/assets/main.3da643cd.js | 20032 | 7.9918 |
| res/static/login/login.bdb7e7c2.js | 598848 | 7.9997 |
| res/xotFullScreen/js/app.1ae30858.js | 1564624 | 7.9999 |
| app.u3a | 13609735 | 7.9991 |

注意：熵只能排除"异或短密钥 / base64 包裹 / 明文直接落地"，**不能**区分 AES-CBC、CTR、ECB —— 三者密文都近似均匀。ECB / CTR 的判定用的是 §3 / §4，不是熵。小文件熵偏低是块数太少的统计偏差（352 B 只有 22 块）。

---

## 6. (e) 大段 0x00 / 非随机区段（IV 前置、明文尾巴）

**所有 43 个 .u3enc**：

- 最长连续 0x00 游程 **0–2 字节**（`index.html` 为 0，多数为 1，1.5MB 与 100KB 两个大文件为 2）。**没有大段 0x00**。
- 末尾 16 字节解密后是纯填充，**没有明文尾巴**；`final()` 全部通过，说明不存在"加密后再拼明文字节"的结构。
- 唯一的前置结构就是 **第 0–15 字节的 IV**（明文存储、每文件不同），见 §0。

**app.u3a** 则完全不同，有明显非随机区段（明文口袋）：

| 偏移 | 内容片段 |
|---|---|
| 4100 | `{\n  "name": "up366",\n  "version": "6.11.0",` —— 明文 package.json |
| 8015 | `  <div class="window">\n    <p id="time" class="time">-1ms</p>` —— 明文 HTML 页面 |
| 10241 | `//ns.adobe.com/xap/1.0/sType/ResourceRef#" xmp:CreatorTool="Adobe Photoshop 21.2 (Macintosh)"` —— 明文 Adobe XMP |

- app.u3a 最长 0x00 游程只有 4 字节（偏移 10925）。
- 全文件魔数扫描：`FF D8 FF E0`(JPEG) = 未找到，`89 50 4E 47`(PNG) = 未找到，`50 4B 03 04`(ZIP) = 未找到，`asar` = 未找到，`{"files"`(asar 头) = 未找到 —— 即**它不是 asar**。
- 前 4,100 字节：可打印率 0.381（随机期望 0.371），无任何 ≥8 字节 ASCII 串；raw-deflate / zlib / gzip / brotli 在 4096 / 4100 / 8192 / 16384 / 65536 / 1048576 六个长度上全部解压失败。

---

## 7. (f) 长度 vs 文件系统里同名明文

文件系统里**不存在**这 5 个 html 的同名明文（全树只有 `resources/res/loading/index.html` 1061 B 和 `resources/audio/index.html` 6610 B，是两个不相关页面）。

但既然能解密，长度关系可以直接给出精确式子（43/43 成立，见 §4）：

```
ct = 16 + 16 * ceil((pt + 1) / 16)
pt = ct - 16 - pad,   pad ∈ [1,16]
```

即每个文件固定多出 **16 字节 IV**，再加 PKCS#7 填充（1..16 字节，平均 8.5）。所以明文长度**不能**从密文长度直接读出，只能确定落在 `(ct-32, ct-17]` 这 16 字节窗口内。

---

## 8. app.u3a 与 .u3enc 是否同一算法

**不是。** 四条数字依据：

1. **长度**：app.u3a 为 13,609,735 字节，`% 16 = 7`。AES 块密码输出必然是 16 的倍数，整文件不可能是同一信封。
2. **存在明文**：偏移 4100（package.json）、8015（HTML）、10241（Adobe XMP）都是长明文串。若为同一套"整文件加密"，不可能有这些明文。
3. **解密尝试全失败**：两把候选密钥（.u3enc 真密钥 `40904d88…`、`6tmxa2VTDs.txt` 的 `39dc2d2d…`）× 三种模式（ECB / CBC-零IV / CBC-前置IV）× 头部 4096 字节，共 12 组，可打印率全部落在 **0.386–0.394**（随机期望 0.371），无一出现 JSON 结构。
4. **重复块指纹相反**：app.u3a 有 4,255 个重复 16 字节块，43 个 .u3enc 合计 0 个。

结论：app.u3a **不套用 .u3enc 的 AES-128-CBC 信封**。但要精确区分两部分，不能笼统说"明文容器"：

- **头部区（约 0–11,000）**：混有长明文（4100 package.json、8015 HTML、10241 Adobe XMP），前 4,100 字节本身是高熵、无 ASCII 串。
- **主体区（16,384 之后，=13,593,351 字节）**：**接近随机，不是明文**。实测：

| 指标 | 主体区实测 | 说明 |
|---|---|---|
| 0x00 频次 | 70,647（均值 53,099） | 超均值 33% |
| 卡方 chi² | 18,616（df=255，均匀期望 ≈255，z ≈ 812） | 显著偏离均匀 |
| 频次前 10 的字节 | 00, 80, 01, 02, 40, 03, c0, 04, 10, 08 | 集中在低字节与单比特位 |
| ASCII 游程 ≥8 | 3,856 个，最长 21 | 随机基线期望 ≈4,900，**低于**随机 —— 无真实文本 |
| PNG/JPEG/ZIP/ELF/WASM/SQLite/OTTO 魔数 | 全部未找到 | 不是常见容器 |
| 17600 起 64 字节 | 含 `04 00 ff 00 2c 00 00 00` 等结构、且与 26944 起整段重复 | 存在真实结构，不是纯噪声 |

即：主体区既不是明文文本/归档，也不是 AES 的均匀输出（AES 输出的卡方应 ≈255），说明它**很可能被另一种（未识别的）编码或加密处理过**，具体算法本次未确定。**不要按明文去 carve app.u3a 的主体区。** 已知排除：asar / zip / jpeg / png / gzip / zlib / raw-deflate / brotli / 8 相位的 `(i%8)+1` 异或。

---

### 8.1 修正（收到 baseline 的 asar 证据后，2026-11-01）

baseline 定位到 app.u3a 就是被改名的 Electron asar（绑定 `electron_common_asar`、源码 `shell/common/asar/archive.cc`、报错串 `Integrity check failed for u3a archive`）。据此复测，上一节的“主体区接近随机”要改成**明文条目与加密条目交替**，卡方 18,616 就是混合样本造成的假象：

- `0..4100`：asar 头部区（未解出，见下）。
- `4100..`：第一个**明文**条目 = package.json（`.json` 不在加密白名单里）。
- `5328..5344`：16 字节 IV；`5344..` 起是 **AES-128-CBC 密文，用的就是同一把 .u3enc 密钥**。解出 `"use strict";"use strict"; var k=Object.create; var c=Object.defineProperty…`，结尾 `0&&(module.exports={DeviceType,LibUp366,SpeakerType});`（webpack chunk，对应 `lib-up366` 模块），约 2.7KB / ~169 块。
- `8023` 起又是**明文** HTML（`ass="window">…<p id="time" class="time">-1ms</p>…loading.gif`）。

所以 app.u3a 的加密**确实复用 .u3enc 的 AES-128-CBC 与同一把密钥**，只是作用域是“白名单后缀的条目”，不是整个文件。上一节“不是同一算法”应改为：**同一密码原语与密钥，但 app.u3a 是容器、按条目选择性加密**。

两条实测警告：

1. **不要用“16 字节对齐处当 IV、看解出的块是否可读”来枚举条目起点**——在 CBC 流内部任意块边界都能“解出可读文本”，我用该法在 5328 之后连续拿到 165 个假阳性。判断条目边界只能用 PKCS#7 或头部。
2. **头部仍未解出**。已排除的尝试（全部失败）：两把已知密钥 `40904d88…` / `39dc2d2d…` 及其 ascii-32B / raw×2 变体 × `aes-128-cbc` / `aes-128-ecb` / `aes-128-ctr` / `aes-256-*` × 多种 IV 位置（iv@0、iv@4、IV 在尾部、以及 ct 结束于 4100 的全部 16 对齐切法），并对每个结果再试 inflate / gunzip——共 15 组，0 命中。
   结构性依据：asar 数据区起点 = `8 + headerSize`；package.json 的 `{` 在 4100，故 headerSize 应为 4092，而实测 `u32le@4 = 952,408`、`u32le@0 = 1,492,020,612`（标准 asar 的 pickle 首字段应为 4）——头部区已被整体改写/加密；且 `4100-4 = 4096 = 16(IV) + 4080(ct)` 是唯一与 CBC 对齐相容的切法，它同样解不出来 ⇒ 头部很可能用**另一把只存在于原生侧的密钥**。

---

### 8.2 一句话总结（供后续引用）

- `.u3enc`：AES-128-CBC + 16B 前置 IV + PKCS#7，全局单密钥 `40904d881995e793c3adec329dedc6b0`，43/43 实测可解。
- `app.u3a`：Electron asar 改名，条目**按后缀选择性**用同一套 AES 加密（.js/.html/.htm/.css/.xml 加密，其余明文）；**头部仍未解出**（需要原生侧密钥）。
- `encr`（ases 插件，非我负责）：固定 8 字节循环异或，无密钥。
---

## 9. `resources/res/6tmxa2VTDs.txt` 的定位

内容 32 个 ASCII hex 字符：`39dc2d2d01becd9258b706e516542396`（= 16 原始字节，长度正好是 AES-128 密钥）。

**它不是 .u3enc 的密钥。** 依据：

- .u3enc 真密钥是从 `up366.exe` 读到的 base64 常量，用它 43/43 解密成功；
- 用 `39dc2d2d…` 作 raw-16B 或 ascii-32B 密钥，在 `aes-128-ecb` / `aes-256-ecb` / `aes-128-cbc(零IV)` / `aes-128-cbc(前置IV)` / `aes-256-cbc(零IV)` / `aes-256-ecb(raw×2)` 六种组合下解 `index.html.u3enc` 头部，全部乱码（可打印率 ≈ 随机）；
- 用它逐字节循环 XOR `app.u3a` 头 64 字节，结果无 `MZ` / `asar` / `PK` / `{` 任何结构；
- 字面量 `6tmxa2VTDs` 在 `up366.exe` 中**不存在**，在 43 个已解密的渲染进程 JS 中也**不存在**。

→ 这把密钥的用途本次未定位，明确留作未决项，不做猜测。

---

## 10. 复现方式

```bash
python tools/probe_u3enc.py            # 完整 JSON
python tools/probe_u3enc.py --brief    # 人类可读摘要
```

脚本只做：读文件 → 字节统计 → 在 `up366.exe` 里 `find` 几个 ASCII 常量取偏移 → 调本机 `node` 跑 `aes-128-cbc`。不写目标目录、不执行客户端、不装第三方包。
