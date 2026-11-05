# 运行时通道可用性

测试时间: 2026-11-01T14:08:58.4253382+08:00

| 通道 | 结果 | 证据 |
|---|---|---|
| --inspect=9229 | False | no listener within 12s |
| --remote-debugging-port=9222 | False | no listener |
| NODE_OPTIONS=--require | False | marker missing |
| ELECTRON_RUN_AS_NODE=1 | True | 22.3.27|16.17.1|undefined |

