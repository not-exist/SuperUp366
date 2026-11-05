# ases 语音评测插件分析

> 分析对象：`extract/plugins/ases`（已解密 JS + 模型资源）与 `C:\Program Files (x86)\up366\client\resources\plugins\ases\ia32\win32\libases.node`（只读，未加载）。
> 方法：静态读 + 自写 Python/PE 解析；未运行客户端、未联网、未写入安装目录。
> 引用约定：`index.js@N` 表示字节偏移；`preload/index.js@N` 指 `extract/app.u3a/dist/preload/index.js` 的字节偏移。压缩成单行的文件按偏移定位，不按行号。

---

## 0. 摘要

| 项 | 结论 | 证据 |
|---|---|---|
| 插件标识 | `name=ases`，`version=4.3.1.0`，browser/renderer 同指 `index.js` | `config.json` |
| JS 导出 | 只有 1 个类 `AsesEngine`（`module.exports = { AsesEngine, default }`） | `index.js@73357`、`index.js@99670` |
| 进程模型 | 渲染进程 JS → `MessagePort` → Electron `utilityProcess` → `libases.node`(N-API) → `lsx-ases.dll`/`api-wavchecker.dll` | `index.js@73386-74900` |
| PE 导出符号 | **0 个**（`DataDirectory[0] = 0/0`，objdump 同为 `00000000 00000000`） | §5.1 |
| JS 可见符号 | 2 个类、23 个成员（AsesEngine 16 + WavChecker 7） | §5.2 |
| 题型目录 | `model/score/` 下 **15** 个目录；其中 **13** 个带 `score-basic-feature.json` | §4 |
| 模型资产 | `.model` **50** 个（LightGBM + libsvm），Kaldi 声学模型 4 套，ONNX 1 个（NISQA） | §3.4 |
| 安装体积 | 303.3 MB（recognition 210.9 / score 50.0 / 二进制 42.2） | `recon/inventory.csv` |
| 纯本地可复现 | 采集、解码、声学打分、特征、模型、分数映射**全部本地**；只有 `net` 文件与 `/feedback` 上报依赖服务端 | §6 |

---

## 1. 插件契约

### 1.1 导出与加载

```js
// index.js@73357（esbuild 生成的 getter 包装，Wg 会给每个 key 加 getter）
var $g = {}; Bg($g, { AsesEngine: () => Gn, default: () => Ug });
module.exports = Wg($g);          // => { AsesEngine, default }  两者同一个类 Gn
```

- `Gn`（`index.js@94654`）`class Gn extends EventEmitter`，即 `AsesEngine`。
- 没有任何 `ipcMain` / `contextBridge` / 消息通道注册；插件不是"注册 ipc 名字"型，而是**导出类 + 单例**型。

宿主侧加载（`preload/index.js@3805000` 附近的 `PluginManager`）：

1. `init()` 扫 `<resources>/plugins` 与 `<saveDir>/plugins`，读每个 `config.json`，按版本号去重（高版本覆盖低版本）。
2. `load("ases")` → `Ome(path)`：若 `index.js.u3enc` 存在，先 AES 解密（`fu()`），再包成
   `(function(exports,require,module,__filename,__dirname,process,Buffer,global){...})` 用 `vm.runInThisContext` 执行，
   **返回 `module.exports.default`**。
3. 调用方固定写 `(await PluginManager.load("ases"))?.inst`（`preload/index.js@3789730`），即插件必须提供
   `static get inst()` 单例（`index.js@95497`）。`config.json` 未设 `instanceName`，所以走 vm 路径而不是 `<script>` 注入。

### 1.2 传输层：两类 Worker + 一种信封

插件内部有两个"远程对象"，都用同一套信封：

| 类 | 载体 | 用途 | 位置 |
|---|---|---|---|
| `cr` | Web Worker（渲染层，`Blob`+URL）/ `worker_threads.Worker`（主进程） | 录音 PCM→Int16、写 WAV（`Ki`） | `index.js@73556` |
| `hr` | `utilityProcess.fork`（主进程侧）或 `ipcRenderer` 拿到的 `MessagePort`（渲染侧） | 承载引擎包装类 `Zi` | `index.js@75135` |

信封（`index.js@74088`，Worker 侧注入的脚本字符串）：

```js
// 请求
{ method: "<方法名>", args: [...], sync: "<hrtime 字符串或 undefined>" }
// 同步响应
{ sync: <同一个 id>, data: <返回值> }         // 抛异常时: { sync, error: e.message }
// 异步/无 sync 的失败
{ code: -1, message: e.message }
```

- `exec(m,...a)`：发出去不管结果（`addData` 用）。
- `execASync(m,...a)`：`hrtime.bigint()` 作 id，`once("sync_"+id)` 等回复；进程退出则以 `{code:-1,message:"进程已退出"}` reject（`index.js@74830`）。
- 渲染层启动流程：`new hr(Zi, AppInfo.uuid())` → `ipcRenderer.on("port")` 收 `MessagePort` → 调
  `AppInfo.createProcess(id, scriptPath)` 让主进程 `utilityProcess.fork` 同一个脚本并把另一端塞进去。
  退出时主进程向页面回 `"port-exit"`，插件 emit `exit` 并自动重建引擎（`createEngine`，`index.js@94923`）。

### 1.3 引擎侧（`Zi`）方法表 —— 真正的 RPC 契约

| method | 参数 | 返回 | 说明 |
|---|---|---|---|
| `init` | `addonPath, modelDir, wcconfigDir, asesLogPath, addonLogPath` | `{code,message}` | `require(addon)`→`new AsesEngine(addonLogPath)`→`Start(modelDir)`；再 `new WavChecker; Start(wcconfigDir)` |
| `reInit` | `addonPath, modelDir, wcconfigDir, asesLogPath` | `{code,message}` | Stop 后重新 Start（code==4 时用） |
| `startParse` | `opts`（见 §2.2） | `{code,message}` | `StartSession` + 装载 net/psdata + 可选 `SpeechIn("wavfile")` |
| `addData` | `ArrayBuffer`(Int16 PCM) | — | `ases.SpeechIn("wav", buf)`；同时喂 `wavChecker.SpeechIn("wav", buf)` |
| `getCheckResult` | `true=speech_end / false=speech_in` | `{code,data}` | 音质/VAD 结果（`Realtime.Volume` 等） |
| `getResult` | — | `{code,data}` | `ases.Result()` → `JSON.parse` |
| `stopParse` | — | — | `ases.StopSession()` + `wavChecker.StopSession()` |

`init` 的错误码语义（`index.js@98358`）：`code<0` 启动引擎失败；`code==1` 引擎 OK、音质检测初始化失败（降级可用）；`code>1` 引擎启动失败。
`startParse` 返回 `4` 表示引擎需要重启（`index.js@98707`）。

### 1.4 对页面暴露的 API 与事件（`AsesEngine` 单例）

| 成员 | 签名 | 说明 |
|---|---|---|
| `get inst` | `static` | 单例 |
| `get Duration` | → ms | `recorder.audioInfo.size / sampleRate * 1000`（按写入的样点数算） |
| `startRecord` | `(opts, scale=1, timeoutSec=300, muteTime=10)` | 先 `parseByNodejs({...opts,wav:""})` 起会话，再开录音；`muteTime` 赋值后从未被读取（死参数） |
| `stopRecord` | `()` | `recorder.stop()` |
| `parseByNodejs` | `(opts)` | 无录音的离线评分入口（异步补评队列用它，`preload/index.js@3786202`） |
| `getResult` | `async ()` | `getResult` RPC + `stopParse` RPC |

事件（页面/preload 消费）：`recording`、`recordstoped`、`score`(`{code,data|message}`)、`recorderror`(string)、`fft`(`[bars], quality, wavcheckJson`)。
页面层再被 preload 转成 `recordstatechange` / `recordfftupdate`（v4）或 `window.onRecorderStateChange` 等回调；状态映射 `u0={0:0,2:1,3:2,4:3,-1:-1,-2:-1,1:1}`（`preload/index.js@3782700`）。

---

## 2. 输入：音频格式与分片

### 2.1 采集链（`kn` 录音器，`index.js@86665`）

```
getUserMedia(audio) → AudioContext({sampleRate:16000}) → createMediaStreamSource
  → createScriptProcessor(4096, ch, ch) → onaudioprocess
  → worker(Ki).addData(Float32 ch0[, ch1])
  → 线性插值重采样到 desiredSampRate → Int16LE → postMessage(ArrayBuffer, transfer)
  → 主线程 ases.exec("addData", buf)  // 直接进引擎
```

| 参数 | 值 | 位置 |
|---|---|---|
| 采样率 | **16000 Hz**（`_r({sampleRate:16e3})`） | `index.js@86982` |
| 位深 | **16 bit signed LE**（`DataView.setInt16(...,true)`） | `index.js@86267` |
| 声道 | **默认 1**（`channel ?? 1`，支持 2 并交错） | `index.js@87220` |
| 分片 | **4096 帧**/次（4096 / 16000 = **256 ms**） | `index.js@86746` |
| 幅度 | `sample * 32767 * scale`，`scale` 默认 **0.333333**，可由 `appConfigs.recorder.scale` 覆盖（限 0.3~1.2） | `index.js@86267`、`preload/index.js@3790000` |
| 落盘 | 44 字节 WAV 头 + 逐块 `writeSync`；RIFF/fmt/data 三块手工拼，`fmt` 里 `bitsPerSample=16` | `index.js@84366` |
| 重采样 | 若 `ctx.sampleRate != desiredSampRate` 用 **线性插值**（不是带限重采样） | `index.js@85226` |

> 注意：`desiredSampRate` 默认 16000，而 ctx 也强制 16000，正常路径下重采样分支不会触发；它只是给"设备拒绝 16k 时"兜底。
> 另：页面参数 `samplingRate` 是**频谱柱数**，不是采样率（见 §2.2）。
> 再：AudioContext 是按 `_r()` 进程内缓存的单例（`index.js@81618`），首次由录音器构造时以 16 kHz 建立；`getAnalyzer` 复用同一个 ctx，所以频谱回调的频点换算用的也是 16 kHz。

### 2.2 `startParse`/`parseByNodejs` 的 opts（唯一的输入契约）

| 字段 | 类型 | 含义 | 消费点 |
|---|---|---|---|
| `wav` | string | 已有 wav 文件路径；空串=实时流模式 | `SpeechIn("wavfile")` |
| `type` | 0/1/2/3 | → `["parta","partb","partc","ASR"]` | `StartSession` |
| `nets` | string[] | `.net` 内容评测模型（每个选项/参考答案一个） | `DataIn("netfile")`（仅 type 0/1） |
| `psdata` | string | 评测参数目录（题面文本/关键词/音素对齐） | `DataIn("path")` |
| `factor` | string | 严格度系数 → `SetParam("score_factor")` | 选不同 `.model` |
| `fakeNets` | string | 调试：`SetParam("fake_net")` 伪造 net | QA 开关 |
| `realScoring` | bool | false 时 `startParse` 直接返回（只录音/只做音质检测） | `startParse` 开头 |
| `vad` | bool | 开 `WavChecker.StartSession`（音量/VAD 实时回调） | `startParse` |
| `fft` | number | 频谱柱数（12/20…） | 只影响 UI 回调 |
| `jsCall`/`newAsec` | — | **完全未被消费**（API 漂移遗留） | 无 |

调用方两处：`preload/index.js@3790000`（`Ct.startRecord` 组装）与 `@3786202`（`AsesResults` 异步补评队列）。
两者都把 `partType==4` 归一到 `type=3`（`type: r.partType==4?3:r.partType`）。

### 2.3 partType → 会话类型

```
partType 0 → parta  （句子朗读 / 麦克风自测，net=ce.net）
partType 1 → partb  （单词/句子跟读）
partType 2 → partc  （仅 psdata，无 netfile）
partType 3 → ASR    （开放题：口语/翻译/复述的自由文本，转写为主）
partType 4 → 3      （归一化）
```

麦克风自测（`extract/res/static/micTest/*.js` → `DeviceTest.startRecordWithParams`，`preload/index.js@3803852`）：
`{recordId, partType:0, nets:["<res>/mictest/ce.net"], fftStyle:1, samplingRate:12|20, diff:1, wav:"<cache>/mictest/test.wav"}`，
录音上限 20 s，得分 <30 判定设备异常。

---

## 3. 评测链路

### 3.1 端到端

```
[麦克风] 16k/16bit/单声道 PCM 4096 帧块
   │  RPC addData(ArrayBuffer)
   ▼
libases.node  SpeechIn("wav", buf)          ──► lsx-ases.dll 会话缓冲
   │                                              │
   │  (实时) wavChecker.SpeechIn → VAD/音量/NISQA 音质（可旁路）
   ▼                                              ▼
StartSession(parta|partb|partc|ASR)          Kaldi 在线解码：MFCC → HCLG 搜索 → 词/音素对齐
   │                                              │  final.mdl / tree / HCLG.fst / lexicon.txt
   │                                              ▼
DataIn("netfile"|"path")                     GOP/准确度/流利度/完整度等打分特征
   │                                              │
   ▼                                              ▼
Result() → JSON 字符串                        score-basic-feature.json: (x-min)*scale 归一化
                                                  │
                                                  ▼
                                              LightGBM / libsvm 回归 → 分数 → score_level 映射
```

### 3.2 步骤 ↔ 资源 ↔ 代码对照

| # | 步骤 | 资源 | JS/二进制位置 | 证据强度 |
|---|---|---|---|---|
| 1 | 采集 & 打包 | — | `kn`/`Ki`（`index.js@84366`、`@86665`） | 直接可见 |
| 2 | 会话建立 | `model/`（根 Configfile 指向子模型） | `Zi.init/startParse`（`index.js@90729,92671`） | 直接可见 |
| 3 | 声学识别 | `recognition/{parta,partb,partc,ASR}`：`final.mdl`(52 MB)、`tree`(2 MB)、`HCLG.fst`(97.9 MB，仅 partb 有)、`lexicon.txt`、`words.text`、`phones.text`、`mfcc.conf`、`online.conf` | `SpeechIn("wav")`→`LsxAsesSessionSpeechIn` | 目录=事实；"Kaldi"=由上述文件名/格式推断（强） |
| 4 | 内容评估 | `Score/content-val-partb/*.net` + 每题下发 `001.net`；`score/ASR/ITN.fst`（逆文本规整） | `DataIn("netfile"/"path")`→`LsxAsesVoasLoadNet` | 直接可见（文件名 + API） |
| 5 | 特征归一化 | `score/<题型>/score-basic-feature.json` | 未在 JS 侧；推断在 `lsx-ases.dll` | 推断 |
| 6 | 模型打分 | `score/<题型>/*.model`（50 个） | 同上 | 文件=事实；"LightGBM/libsvm"=文件名 + `libopenblas/libgfortran` 依赖推断（强） |
| 7 | 音质检测（旁路） | `wcconfig/`：`volume/*.conf`、`webrtcvad/vad.conf`、`nisqa-info/nisqa.onnx` + `onnxruntime.dll` | `WavChecker.*`→`api-wavchecker.dll` | 直接可见（`WavCheckerDef` 版本串 `V3.4.0 / 2024-03-28`） |
| 8 | 分数→档位 | `score_level`、`score_range_*` | 推断同 5/6 | 配置可见，落点推断 |

### 3.3 严格度 → 模型文件

`factor` → `SetParam("score_factor")`，按后缀选模型：

| 后缀 | 语义 | 出现在 |
|---|---|---|
| `-normal` | 标准 | keysentence / keyinformation / retell / mindmap / pc-*-retell / readaloud |
| `-strict` / `-extremely_strict` | 严格 | 同上 |
| `-lenient` / `-extremely_lenient` | 宽松 | 同上 |
| `-harsh` | 极严（readaloud 用 `extremely_strict`） | keysentence/keyinformation/retell/mindmap/pc-* |
| `-loose` | 极宽（readaloud 用 `extremely_lenient`） | 同上 |
| `-comp` | 对比/补偿 | `lightgbm-auto-comp.model`（keyinformation / mindmap / pc-gd） |

对应 `score-basic-feature.json` 里的 `score_range_{normal,strict,lenient,harsh,loose}` 五档。

### 3.4 模型清单（来自 `recon/inventory.csv`，extract 内只有 JSON/txt）

| 目录 | .model 数 | 框架（按文件名） | 备注 |
|---|---|---|---|
| readaloud | 14 | svm_accuracy_{normal,strict,lenient}(4.8/8.5/6.1 MB)、accuracy_svm_junior_sentence_*(1.3/0.2/1.1 MB)、lightgbm_paragraph_recite_*(5)、svm-fluency/svm-sentence/svm-word | 唯一同时有 SVM 与 GBDT 的题型 |
| keyinformation | 6 | lightgbm-auto-{comp,normal,strict,lenient,harsh,loose} | |
| pc-bj-mindmap | 6 | 同上 | 只有 `lightgbm-auto-comp.model` 与 keyinformation 同 sha256，其余 5 个不同 |
| pc-gd-information-retell | 6 | 同上 | **6 个模型的 sha256 与 keyinformation 逐一相同**（整目录复用） |
| keysentence | 5 | lightgbm-auto-* | 无 comp |
| pc-listening-retell | 5 | lightgbm-auto-* | |
| retell | 5 | **libsvm-auto-*** | 唯一用 libsvm 的题型 |
| answer / hint / translation | 各 1 | lightgbm.model | 单模型 |
| choice / dailyexpression / promptzh-choice | 0 | — | 纯规则 + 阈值 |
| ASR | 0 | — | 仅 `ITN.fst` |
| content-val-partb | 0 | — | 仅 `AD001.net` 样例 |

---

## 4. 特征表（`score-basic-feature.json`）

> 文件本身带 `encr` 外壳（见附录 A）。13 个题型各一份，字段高度复用。

### 4.1 通用字段

| 字段 | 结构 | 含义（推断） |
|---|---|---|
| `score_level` | `{"1":[1.5,2.0],...,"5":[-0.5,0.0]}` | 5 档分数区间，把模型原始分切成 1~5 级；**13/13 个题型都有，且数值完全相同** |
| `target_mark_system` | number | 目标分制（100.0） |
| `init_mark_system` | number | 初始分/基线（1.0 / 2.0 / 10.0 三种） |
| `raw_score_threshold` | number | 原始分阈值（65.0），低于则判不合格 |
| `score_range_<题型>` | 5 组 4 元素数组 | 按严格度的 4 段分数线（第 1 段=满分档下界） |
| `EXCEPT_WORD` | string[24] | 不计错的虚词表（for/to/of/a/an/the/and/is…） |
| `select_top_n_texts_threshold` | 50 | 检索参考文本的候选数 |
| `f_similarity_threshold` | 0.5 | 文本相似度阈值 |
| `feature_transform` | `{scale[],min[]}` | 特征标准化：`(x-min)*scale` |
| `addscore_feature_transform` | `{scale[6],min[6]}` | 加分项特征标准化（`[1.14285714, 0.01670224, 1.25, 1, 1, 1]`，三种题型完全一致） |
| `addscore_switch` | bool | 加分项开关 |
| `threshold_combining_reference` | 0.7 | 学生答案与参考文本融合阈值 |

### 4.2 按题型分组

| 题型目录 | 字段数 | 特有/差异字段 |
|---|---|---|
| `readaloud` | 31 | `pron_score{mean[14],scale[14]}`、`sentence_flu_score{mean[6],scale[6]}`、`specific_words_score[42]`、`chunk_pron_score{phrase_linreg_weight/bias × normal/strict/lenient}`、4 套 `*_paragraph_score_model{coefficients,bias}`（gd/default/bj/gz_jiaocai）、`sentence_recite_score_model`、`score_rate[0.3,0.25,0.45]`、`F_SimilarScoreThreshold(7.5)/F_DuductionScoreThreshold(85)/Max_Deduction(30)` 及 `_Dubbing` 变体(8.3/75/40)、`incoherence_threshold(0.6)`、`high_range(0.15)/low_range(0.25)`、`word_number_short(4)/long(9)`、`word_short_threshold(0.75)/word_long_threshold(1.3)`、`sense_err_threshold(0.09)`、`paragraph_addscore_{switch,model_coef[6],model_intercept(30.633),hparams[9]}` |
| `answer` | 8 | `score_range_answer`、`EXCEPT_WORD`、`raw_score_threshold` |
| `promptzh-choice` | 8 | 与 answer 同构，仅 `score_range_promptzh_choice` |
| `hint` | 6 | `score_range_hint` |
| `translation` | 6 | `score_range_translation`，阈值量级是 **0~2**（[1.75,1.25,0.75,0.25]），与其它题型的百分制明显不同；但 `target_mark_system` 仍是 100 → 推断是先按 2 分制切档再放大到 100 |
| `keyinformation` | 7 | `feature_transform[8]`、`addscore_*`、`threshold_combining_reference` |
| `pc-gd-information-retell` | 7 | 与 keyinformation **逐字节同构**（feature_transform 完全相同） |
| `pc-bj-mindmap` | 7 | 同构，仅 `feature_transform.scale/min` 不同 |
| `keysentence` | 5 | `feature_transform[9]`，无 addscore |
| `pc-listening-retell` | 4 | `feature_transform[11]`，无 `threshold_combining_reference` |
| `retell` | 3 | 只有 `score_level/target_mark_system/init_mark_system` |
| `choice` | 3 | 只有 `score_level` + 两个相似度阈值（最简） |
| `dailyexpression` | 5 | `target_mark_system=100`、`init_mark_system=1.0` |
| `ASR` / `content-val-partb` | — | **无** `score-basic-feature.json` |

### 4.3 readaloud 归一化参数（可直接复算）

```
pron_score.mean  = [0.8424806, 0.14360981, 0.01390959, ...]   (14 维)
pron_score.scale = [0.1953304, 0.18236997, 0.0566683, ...]     (14 维)
sentence_flu_score.mean/scale (6 维)
chunk_pron_score:
  phrase_linreg_weight_normal  = [7.24019359, -3.2126987, -2.73797336, 3.47094929], bias -9.29116285
  phrase_linreg_weight_strict  = [6.13369387, -4.65713971, -3.04344281, 2.30008002], bias -5.87848175
  phrase_linreg_weight_lenient = [8.36494797, -2.31614582, -1.38482631, 3.04651826], bias -8.03651615
paragraph_score_model (coefficients 5 维 + bias):
  default [0.77153435,0.05276708,0.18937178,-23.63692796,-15.74325964] bias 0.42531665
  bj      [0.77266539,0.05508793,0.18843264,-23.51403671,-15.79654803] bias 0.1992771
  gd      [0.76163207,0.05030321,0.189155,  -11.92673387, -9.22083966] bias -0.01271433
  gz_jiaocai [1.37385152,-0.15090083,0.14695441, 1.00945767, -6.99437855] bias -31.2917876
```

> 4 套 paragraph 模型分别对应 广东/北京/贵州教材 与默认；选择逻辑不在 JS 里（推断由 psdata 的题源字段触发）。

### 4.4 其余模型侧词典（不属于"特征表"，但同目录解密后内容如下）

| 文件 | 实际内容 | 与文件名预期的差异 |
|---|---|---|
| `feature/feature_info.json` (36.8 KB) | `empty_wrd[203]`、`empty_wrd_for_dfbc[157]`、`stop_wrd[179]`、`useless_wrd[416]`、`abbreviation_list[47]`（正则替换对，如 `( \|^)can't` → ` can not `）、`word_normalization[864]`（英式/美式拼写对，如 `analyse,analyze`） | **不是特征名表**，是 NLP 归一化/停用词典 |
| `number-engine/number_engine_info.json` (755 B) | `base_constant`(thousand/million/billion)、`fenmu_dict`(序数/分母 first…tenths)、`force_number_small`、`number_constant[20]`、`number_constant2[10]` | 数字→英文读法（ITN 逆变换用） |
| `recognition/partb/lexicon.txt` (2.37 MB, 62,915 行) | CMUdict 格式 + 位置标记：`A EY1_S`、`A'S EY1_B Z_E`、`<UNK> SPN_S`、`<eps> SIL` | `_B/_I/_E/_S` 是词内音素位置标签（Kaldi 词位建模） |
| `recognition/partb/words.text`、`phones.text` | **extract 中不存在**（只提交了 lexicon.txt）；原始安装目录里有，`phones.text` 4210 B、`words.text` 1.83 MB | 提取时未包含非 JSON/TXT 大文件；引用请以 inventory 为准 |
| `model/stemming/fix_word.text` (33 KB) | 未读到（extract 未收录），仅从文件名推断是词干修正表 | — |

> 结论：任务里"score-basic-feature.json 是特征名表"的假设不成立——它是**打分超参表**；真正的"特征名"没有以文本形式出现在本地，只存在于 `lsx-ases.dll` 的代码里。

---

## 5. `libases.node` 符号

### 5.1 PE 概况（`objdump -p` + 自写解析，两者一致）

| 项 | 值 |
|---|---|
| 格式 | PE32 i386 DLL（`.node`），428,032 B，5 节（.text .rdata .data .rsrc .reloc） |
| 时间戳 | 2024-11-27 17:47:09（1732700829） |
| 链接器 | 14.42（VS2022），`DllCharacteristics = DYNAMIC_BASE|NX_COMPAT` |
| PDB | `E:\up366item\up366plugins\ases\build\Release\libases.pdb`；源 `...\ases\src\main.cpp` |
| **导出表** | **`DataDirectory[0] = 0x00000000 / 0x00000000` → 0 个导出符号**；objdump 输出 `Entry 0 00000000 00000000 Export Directory`。文件内也搜不到 `napi_register_module_v1` / `node_module_register` 字符串 |
| 导入 | `KERNEL32.dll` 87 项；`lsx-ases.dll` **按序号导入 16 个**；`api-wavchecker.dll` **按序号导入 12 个** |
| 延迟导入 | `node.exe`（41 个 `napi_*`：`napi_define_class`、`napi_wrap`、`napi_get_cb_info`、`napi_module_register` 等）→ 说明用 **node-addon-api(C++)** 写、靠 `napi_module_register` 自注册 |

> 交付给 team-lead 的"导出符号数"：**PE 导出表 0 个**；真正可用的符号面是 N-API 注册的 **2 个类 / 23 个成员**（下表）。

### 5.2 JS 可见符号（`.rdata` 类名+方法名表，`0x5bbf4..0x5cae0`）

`AsesEngine`（16）：

| 方法 | 映射到的 C API（`.rdata` 日志串） | 说明 |
|---|---|---|
| `Version()` | — | 版本号 |
| `Start(path)` | `LsxAsesStart` | path=model 根目录 |
| `Stop()` | `LsxAsesStop` | |
| `SetAsesParam(name,val)` | `LsxAsesSetParam` | `log-open`/`log-level` |
| `GetParam(name)` | `LsxAsesGetParam` | |
| `StartSession(part)` | `LsxAsesSessionBegin` | `parta/partb/partc/ASR` |
| `StopSession()` | `LsxAsesSessionEnd` | |
| `SetParam(name,val)` | `LsxAsesSessionSetParam` | `md5`/`score_factor`/`fake_net` |
| `DataIn(type,data)` | `LsxAsesSessionDataIn` | `netfile`/`path`，接受 string 或 DataView |
| `SpeechIn(type,buf)` | `LsxAsesSessionSpeechIn` | `wav`(流)/`wavfile`(路径) |
| `Result()` | `LsxAsesSessionGetResult` | 返回 JSON 字符串 |
| `GetError(code)` | —（本地错误表） | 未命中→`"Unknown error"` |
| `LoadNet()` | `LsxAsesVoasLoadNet` | .net 装载（插件未直接调，走 DataIn） |
| `GetText()` | — | 未使用 |
| `GetTextSplitCount()` | — | 未使用 |
| `IsKeyWord()` | — | 未使用 |

`WavChecker`（7）：`Start(wcconfig)`、`Stop()`、`SpeechIn("wav",buf)`、`Result("speech_in"|"speech_end")`、`GetError(code)`、`Duration`、`GenerateFingerprintJson`（后两者插件未调用）。
返回结构见 `index.js@89997` 的 `wavCheckerDef`：`Realtime{Volume,VoiceState,RealtimeSilenceNum,...}` / `Switch{UseConfigFile,RealtimeVolumeModel,FinalVolumeModel,WebrtcvadStateModel,FingerprintIdentification,LanguageIdentification}` / `Final{Volume,Damage,Language,Cheat,Current,Snr,Event,Duration,VoiceMeanDb,SilMeanDb,TruncationCount,...}`，`Version: "V3.4.0"`。

### 5.3 内部 C API 面（按序号导入，名字来自 `.rdata` 日志串）

`LsxAsesStart / LsxAsesStop / LsxAsesSetParam / LsxAsesGetParam / LsxAsesSessionBegin / LsxAsesSessionDataIn /
LsxAsesSessionSetParam / LsxAsesSessionGetResult / LsxAsesSessionSpeechIn / LsxAsesSessionEnd / LsxAsesVoasLoadNet`（11 个已知）
+ `api-wavchecker.dll` 12 个未命名序号函数。

依赖 DLL（inventory）：`lsx-ases.dll` 4.8 MB、`api-wavchecker.dll` 727 KB、`onnxruntime.dll` 7.6 MB、
`libopenblas.dll` 25 MB、`libgfortran-3.dll` 1 MB、`libgcc_s_*`、`libquadmath-0.dll`、`libwinpthread-1.dll`。
（openblas+gfortran ⇒ 引擎侧是 MinGW 编译的 Kaldi/数值库；onnxruntime ⇒ NISQA 音质模型。）

---

## 6. 可离线复现性

| 环节 | 纯本地可复现？ | 依赖/缺口 |
|---|---|---|
| 录音采集（16k/16bit/单声道/4096 帧） | ✅ 完全 | 只需 WebAudio + `getUserMedia`；WAV 头可手写（44 B 模板见 `index.js@84366`） |
| WAV → Int16 → 引擎喂数据 | ✅ | `SpeechIn("wav")`/`("wavfile")` 都在本地 |
| 声学识别（Kaldi） | ✅ **本地**（210.9 MB 模型全在安装目录） | 需要 `lsx-ases.dll`（Kaldi 应静态链入该 DLL，openblas/gfortran 由同目录 DLL 提供 —— 推断）；模型未加密 |
| 内容评估（GOP/对齐/打分） | ✅ **本地** | 需要 **`.net` 文件**（每道题的参考文本/音素图），来自题目包或服务端下发 |
| 特征归一化 + GBDT/SVM 打分 | ✅ **本地**（50 个 `.model`；超参 JSON 只需 8 字节头 + 周期 8 的异或） | 权重未加密，可离线用 LightGBM/libsvm 复算 |
| 严格度切换 | ✅ | `score_factor` 选不同后缀模型 |
| 音质/VAD（NISQA） | ✅ | `nisqa.onnx` + `onnxruntime.dll` 均在本地 |
| **`.net` 题目模型获取** | ❌ **依赖服务端** | `preload/index.js@3797000`：`netFiles` 是 URL，`https://...` 拉取后落 `<cache>/download/net/<md5>/001.net`；缺失则直接报"启动录音失败，评分文件下载失败"并 `c(4)` 终止 |
| 分数上报/失败补评 | ❌ 服务端 | `uploadFailedAses` 打 7z 上传 + `saveSpokenFeedback` |
| 插件自身升级 | ❌ | `modules/plugin-ases.mod` 增量包机制 |

要点：
1. **打分链路 100% 本地**，服务端只负责发题（含 `.net`/psdata）和收结果。断网状态下只要题目包已缓存（`net` 已下载），可以完整复现评分。
2. 复现的硬门槛是 `lsx-ases.dll` + 4 套 Kaldi 模型 + 50 个 `.model`，这些**都在安装目录明文**（不像 JS 有 `.u3enc`），只有 `score-basic-feature.json`/`feature_info.json` 这类配置套了一层 8 字节异或壳。
3. `recognition/partc` 与 `recognition/ASR` 目录下**只有 `Configfile`**（无 `final.mdl`），推断它们复用 parta/partb 的声学模型，具体映射需读 `Configfile` 原文（未在 extract 中）。
4. `wcconfig/`（VAD/音量/NISQA 配置）与 `files.list`（23 KB 自检清单）同样只在安装目录，extract 未收录。

---

## 附录 A. `encr` 模型文件解密（新发现，`crypto-notes.md` 未收录）

模型目录下所有 `*.json/*.txt` 都带一个轻量壳，**不是 AES**：

```
offset 0..3   : "encr"
offset 4..7   : uint32 LE = 明文长度 N（文件总长 = 8 + N，16/16 个文件全部成立）
offset 8..    : payload[i] = plain[i] XOR ((i % 8) + 1)
```

Python 三行：

```python
b = open(p,"rb").read(); n = struct.unpack_from("<I", b, 4)[0]
plain = bytes(b[8+i] ^ ((i % 8) + 1) for i in range(n))
```

验证：16/16 个文件解密后均为合法 UTF-8，JSON 全部可 `json.loads`。第一个字节 `0x7a ^ 1 = '{'` 即密钥起点。

## 附录 B. 证据索引

| 结论 | 位置 |
|---|---|
| 导出只有 AsesEngine | `extract/plugins/ases/index.js@73357`、`@99670` |
| 单例 + 事件 | `index.js@94654`（`Gn=class extends Pu.EventEmitter`）、`@95497`（`static get inst`） |
| RPC 信封 | `index.js@74088`（注入脚本）、`@74830`（`execASync`） |
| 引擎方法体 | `index.js@89872`–`@94595`（类 `Zi`） |
| 16k/4096/Int16/scale | `index.js@86746`、`@86267`、`@84366` |
| opts 组装 | `extract/app.u3a/dist/preload/index.js@3790000`（`Ct.startRecord`）、`@3786202`（异步队列） |
| 麦克风自测 | `preload/index.js@3803852`（`DeviceTest.startRecordWithParams`） |
| 插件加载/版本裁决 | `preload/index.js@3805300`（`PluginManager`、`Ome`） |
| 完整文件树/体积/sha256 | `recon/inventory.csv` |
| PE 导出为 0 | `objdump -p libases.node` → `Entry 0 00000000 00000000` |

## 附录 C. 未确认 / 需下游验证

1. `feature_transform` / `score-basic-feature.json` 究竟在 `lsx-ases.dll` 的哪个函数消费（无符号、无导出，需反汇编定位 `LsxAsesSessionGetResult`）。
2. `partc`/`ASR` 的 `Configfile`（343/341 B）内容 → extract 未收录，需从安装目录另取。
3. `.net` 文件格式（`AD001.net` 10 KB / `ce.net`）未分析：它是"每题内容模型"，格式未知。
4. 4 套 `paragraph_score_model` 的选取条件（gd/bj/gz_jiaocai/default）未见任何 JS 分支，应在 psdata 里。
