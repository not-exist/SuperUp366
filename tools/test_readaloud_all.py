# -*- coding: utf-8 -*-
"""逐句跟读提交链路的核心语义测试（一次跑完）。

只测「算错就整包被拒 / 音频丢失」的核心语义，不测 UI 与网络：
  1) ut 构造 —— 与本地真实历史报文逐字节比对（tools/fixtures/real-submit-payloads.json）
  2) objectName —— 与客户端 buildOSSObjectName 公式逐字符比对

跑法：python tools/test_readaloud_all.py
"""
import os
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
MODULES = ["test_readaloud.py", "test_objectname.py"]


def main():
    failed = 0
    for m in MODULES:
        print("--- %s" % m)
        r = subprocess.run([sys.executable, os.path.join(HERE, m)], capture_output=True, text=True)
        sys.stdout.write(r.stdout)
        if r.returncode != 0:
            sys.stdout.write(r.stderr)
            failed += 1
    if failed:
        print("\nFAILED: %d/%d" % (failed, len(MODULES)))
        sys.exit(1)
    print("\nall passed: %d/%d" % (len(MODULES), len(MODULES)))


if __name__ == "__main__":
    main()
