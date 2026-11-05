# 单词PK 成绩提交 —— 二进制原生层安全分析

> 时间：2026-11-01　范围：`C:\Program Files (x86)\up366\client` 下的原生二进制（只读、未启动客户端、未联网）
> 工具：rea MCP（`inspect_native_exports` / `inspect_pe_imports`）+ GNU objdump 11 + 自写只读扫描脚本（`tools/sec_*.py`）
> 关联：JS 侧结论见 `recon/security.md`；服务端去重结论见 `recon/dup-submit-security.md`

---

## 0. 结论（可直接引用）

| # | 问题 | 结论 |
|---|---|---|
| 1 | 原生层对 `httpRequest` 请求体是否签名 / HMAC / 加密 / 二次编码？ | **没有。** 提交不经过任何自有原生代码：真正的发起方是 Chromium 网络栈（`Network Persistent State` 的 HTTP/2 origin 记录为证），三个 `.node` 原生模块全部没有网络导入、没有签名调用点；主程序是改造版 Electron，改造内容只有 `u3a/u3enc` 资源解密。原生层能做的只有"不改 body"。 |
| 2 | 原生层是否出现 `wordsbtl` / `word-king` / `submit` 字符串？ | **完全没有。** 整个安装目录递归扫 ASCII + UTF-16LE，`wordsbtl` / `PC-Up366-Student` / `x-app-name` 命中数均为 0。它只出现在 Chromium 自己的运行时数据里（`Network Persistent State` 的 origin 表、V8 code cache 的 JS 字符串池），属于 JS 配置 + HTTP/2 会话记录，**不是白名单、不是日志、不是校验逻辑**。 |
| 3 | 原生层有"只能提交一次"保护（本地计数 / 一次性 token / 防重放 nonce）？ | **没有。** 原生模块里 `nonce` / `replay` / `hmac` / `submit` 全 0 命中（`token` 的 2 次命中是 `OpenProcessToken`/`GetTokenInformation`），也没有任何提交态 API。唯一的判重在服务端（`uid + studyTaskId`，见 `dup-submit-security.md`）。 |

一句话：**原生层是"哑管道"，安全边界全部在服务端；客户端没有、也不可能在二进制里藏签名。**

---

## 1. 证据

### 1.1 原生资产清单（含指纹）

| 文件 | 大小 | SHA-256 | 角色 |
|---|---|---|---|
| `up366.exe` | 136,310,520 | `66d2a18c4489b8f53d56b4e96db83f2f33c4e861990612130e31d3f435c0e881` | 改造版 Electron 主程序 |
| `lib/ia32/win32/libup366.node` | 545,520 | `4e4873eb1d4ba960438acd2969e074e61105d9254b06a23e5e931e7f92409917` | Windows 系统工具型 N-API 插件 |
| `lib/ia32/win32/libsqlite.node` | — | — | SQLite 绑定（对照组） |
| `resources/plugins/ases/ia32/win32/libases.node` | 428,032 | — | 口语评测插件（lsx-ases.dll / api-wavchecker.dll） |
| `lib/ia32/win32/computerInfo.exe` | — | `ae824c2f4...` | **MachineCode 生成器**（独立进程） |
| `lib/ia32/win32/runtime-check/up366RunTimeCheck.exe` | — | — | 运行环境自检（唯一带签名能力的组件） |
| `resources/app.u3a` | 13,609,735 | `b2f5cb8f...` | 全加密应用包，熵 7.999/8.0，无 `PK`/`package.json` 明文 |

### 1.2 问题 1：请求体无签名（五组独立证据）

**(a) 三个原生模块都没有网络栈 → 物理上无法参与 HTTP**
`rea.inspect_pe_imports` + 自写解析（含 delay-import）结果：

- `libup366.node`：KERNEL32(112) / USER32(18) / GDI32(1) / ADVAPI32(14) / SHELL32(3) / ole32(3) / CFGMGR32(2) / SETUPAPI(4) / VERSION(3)，delay-import 仅 `node.exe`。
  **无 WS2_32 / WinHTTP / WinINet，无任何 `Crypt*` / `BCrypt*`（ADVAPI32 只用到注册表与服务控制 14 个函数）。**
- `libases.node`：KERNEL32(87) + `lsx-ases.dll`(16) + `api-wavchecker.dll`(12)，同样无网络、无加密。
- `libsqlite.node`：仅 KERNEL32(121)。

**(b) `libup366.node` 的实际能力 = 桌面运维，不是网络**
其导出 API 名表（`.rdata`，文件偏移 `0x6f8e4`–`0x6fa5c`）完整列出：
`getDiskUsage / getDiskList / uninstallUsbDisk / getVolume / setVolume / getMute / setMute / registerFileAssociation / createLinkFile / getProcessListInfo / getFileVersion / switchToThisWindow / autoSetScreenSize / processIsElevated / windowsServiceIsRunning / startupWindowsService / exitEvent / disableKey / enableKey / getScreenScaleFactor / disableIME / autoTopWindow / stopAutoTopWindow`
源码路径字符串（`0x6f848`）：`E:\up366item\up366plugins\up366\src\main.cpp`。
→ 该插件是 U 盘卸载 / 音量 / 进程列表 / 置顶窗口那一类工具集合，**没有任何 http、签名字段**。

**(c) 通信本身走 Chromium 网络栈，不经过 `.node`**
`%APPDATA%\up366\Network\Network Persistent State` @2230 记载：

```json
{"anonymization":[],"server":"https://wordsbtl-api.up366.cn","supports_spdy":true}
```

该文件仅由 Chromium 网络服务写入（`supports_spdy` = HTTP/2 能力协商）。自有原生 HTTP 客户端不会在这里留痕 —— 说明提交请求由 Electron 的 net/XHR 发起，原生层只是被调用的 `.node`，且它们没有网络能力。

**(d) 主程序层面：`up366.exe` = 改造版 Electron，改造只涉及资源包解密**
PDB 路径 @130,757,275（`0x7cb329b`）：`E:\up366\electron\electron\src\out\release32\electron.exe.pdb`。
导入：WS2_32(51)、WINHTTP(14，`WinHttpGetProxyForUrl` 代理探测与 `WinHttpOpenRequest/SendRequest/ReadData` 请求族并存，属 Chromium 自带 WinHTTP 适配)、CRYPT32(32，只有 `CryptProtectData/UnprotectData`（OSCrypt）与 `CryptVerifyCertificateSignatureEx/CryptAcquireCertificatePrivateKey`（证书校验）)、Secur32(10)、ncrypt(delay)。
**没有任何 `CryptCreateHash` / `CryptSignHash` / `CryptSignMessage` 之类的签名原语调用。**

需要说清的边界：哈希/HMAC 能力由内置 BoringSSL（Node crypto，`createDecipheriv` 可见于 @124,261,154）提供，**"没有 crypto 导入"不等于进程不能算 HMAC**。真正成立的是两点：
① 二进制里没有任何业务字符串（`wordsbtl` / `PC-Up366-Student` / `x-app-name` 全 0），拿不到"该进程给某接口签名"的调用点；
② 请求经 Chromium 网络栈发出（1.2c），body 由 JS 决定后原样上线，原生侧没有插入签名头的痕迹。
因此签名若要存在，只可能在 JS 层；且服务端不要求（已用纯 Python 无签名提交成功）。

**(e) 唯一"二次编码/加密"发生在资源层，与请求无关**
改造点在 `asar_util.cc`（UTF-16 串 @112,131,516 = `0x6aefdbc`）：
`...\electron\shell\common\asar\asar_util.cc — Integrity check failed for u3a archive`
以及内嵌的加载器 JS @128,155,148（`0x7a37e0c`）：

```js
const d=/\.u3a/i, y=/\.u3enc/i, _={".xml":!0,".js":!0,".html":!0,".htm":!0,".css":!0},
getEncFileName=t=>_[a.extname(t).toLocaleLowerCase()]?t+".u3enc":t,
... t.lstatSync=(t,e)=>{t=e?.orig?t:getEncFileName(t); ...}
```

配合 `createDecipheriv`(@124,261,154 = `0x7681322`，Electron 内置 Node crypto/BoringSSL)，即 **JS/HTML/CSS 资产落盘加密，运行时透明解密**。这是唯一在原生层做加解密的地方，作用对象是磁盘资源，不是 HTTP body。

### 1.3 问题 2：`wordsbtl` 字符串归属

- 递归扫描 `C:\Program Files (x86)\up366`（ASCII + UTF-16LE 双模式）：`wordsbtl` = **0**，`word-king`/`wordking` = **0**，`PC-Up366-Student` = **0**，`x-app-name` = **0**。
- 出现位置仅两处，且都在 `%APPDATA%\up366`：
  1. `Network\Network Persistent State` @2230 —— HTTP/2 origin 记录（见 1.2c）；
  2. `Code Cache\js\683d5efe10214712_0` @596 —— V8 编译产物里的字符串池，与 `report-api / mall-api / course-api / study-api / word-manage / wrongnote-api / book-api / self-api / xot-api / daily-api / qlib-api / homework-api / sso / msg-api / pay / live-api` 并列出现：

     ```
     ...self-api.up366.cn....$Qg.!......wordsbtl-api.up366.cn.....Qe.VT.....xot-api.up366.cn...
     ```

  → 主机名表来自 **JS 代码里的硬编码列表**（`.u3enc` 解密后执行，再被 V8 缓存），属"接口地址配置"，不属于白名单校验，也不是日志。
- `/client/wordsbtl/student/submit` 这类路径在安装目录内外均无明文命中，说明路径是在 JS 里拼接的（与"纯 Python 不带签名可提交"一致）。

### 1.4 问题 3：无客户端一次性保护

- 原生侧关键字扫描 `nonce / replay / secret / hmac / submit / aes / encrypt` 在 `libup366.node` 全为 0；`token` 的 2 次命中是导入表里的 `OpenProcessToken / GetTokenInformation`（Windows 权限令牌）。
- `libup366.node` 的 23 个导出里没有任何与"成绩/任务/提交"相关的 API（见 1.2b）。
- `%APPDATA%\up366` 全域扫描 `submitFlag / hasSubmit / isSubmit / x-sign / nonce` 均 0 命中；`sign` 的命中经上下文核对全部是**语言学**含义（`defaultSigns` / `defaultMayWordSigns` / `isSpecialSign`，音标/词符场景），不是签名算法。
- 交叉验证：`recon/dup-submit-security.md` 已实测——改本地 SQLCipher 里的 `studytaskrecord.score` 后重提仍是 `-13 重复作答`，判重键在服务端 `(uid, studyTaskId)`。**结论一致：客户端没有可绕的"已提交"开关，因为它根本没有这个状态。**

### 1.5 附：客户端里唯一具备签名能力的二进制（与提交无关）

`lib/ia32/win32/runtime-check/up366RunTimeCheck.exe`：
- 导入 WS2_32(35)、CRYPT32(10)、`libcurl.dll`，ADVAPI32 含 `CryptAcquireContextA / CryptCreateHash / CryptSignHashA / CryptExportKey`。
- 内置 URL 只有环境自检与更新域：`setup-api.up366.cn/front/health-status`、`/front/appversion/latestVersion`、`fs.up366.cn/fileinfo`、`logs-api.up366.cn/front/uploadfile`、`www.baidu.com`（测速）。**无 `wordsbtl`。**
- 判断：这是安装/运行环境体检 + 更新的独立进程，签名能力服务于它自己的上报链，**不在单词PK 提交链上**。

顺带定位 `clientId: <MachineCode>` 的来源：`computerInfo.exe` 含 UTF-16 `MachineCode` @974,484（`0xede94`），并查询 `Win32_BaseBoard` / `Win32_BIOS` / `Win32_OperatingSystem`（WMI，`Manufacturer` 等字段）——硬件指纹由独立进程算好，经 IPC 交给 JS 放进请求头；该进程无加密导入，只额外用 `netapi32!Netbios` + `wsock32!send` 取网卡地址，不含任何业务 API 字符串。

---

## 2. 复现方式（只读）

```bash
python tools/scan_dir.py "C:/Program Files (x86)/up366" 'wordsbtl,pc-up366-student,x-app-name,submit'   # 全目录 ASCII+UTF-16 计数
python tools/sec_imports2.py "<某二进制>"                                                              # 导入表 + 加解密/网络函数审计（含 delay-import）
python tools/pe_layout.py "<某二进制>"                                                                 # 节表 / 数据目录 / 熵
python tools/sec_binctx.py "<文件>" <关键字> <上下文宽度>                                              # 命中位置上下文
objdump -p "<某二进制>" | grep -A5 'Export Directory'                                                  # 导出表交叉验证
```

REA 侧：`open_binary` 绑定 → `inspect_native_exports` / `inspect_pe_imports`（`binary_overview`、`inspect_binary_layout` 因未配置 ghidra/ida/hopper 不可用，与结论无关）。

---

## 3. 未验证项

1. **`libup366.node` 无导出表是个异常**：PE 数据目录 `Export` = (RVA 0, Size 0)，objdump 亦报 `Entry 0 00000000 00000000 Export Directory`；对照组 `libsqlite.node` 正常导出 `node_register_module_v110@0x10001720`。按标准 `process.dlopen` 语义该模块**无法被 require 成功**。是否实际被加载、还是遗留/被替换，未动态验证（本次禁止启动客户端）。
2. `app.u3a` 熵 7.999，静态不可读；应用 JS 由其他队友通过解密链取得（`extract/app.u3a`）。本文只断言"二进制侧没有签名"，**不覆盖 JS 桥里是否做了与签名等价但无密钥意义的加工**（如字段重排/去空格）——那部分请在 `main-window.js` / `preload/index.js` 的 `httpRequest` 实现上核对。注意 `dist/main/index.js` 里 `httpRequest` 的命中是打包进来的 aws-sdk 内部字段，不是业务桥。
3. `up366RunTimeCheck.exe` 的 `CryptSignHashA` 实际签名对象（机器码？请求？）未反编译确认；已确认它不含 `wordsbtl` 字面量与路径。
4. 7 个请求头的注入点未在二进制中定位到（无任何字面量）——推断为 JS 层注入，与 `supports_spdy` 证据一致。
