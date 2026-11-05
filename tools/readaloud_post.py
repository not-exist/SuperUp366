# -*- coding: utf-8 -*-
"""直接把已构造好的行 POST 到 /client/task/score/submit/v2（分支 A）。

走客户端 `AppInfo.httpRequest`：带 7 个公共头、登录态、ST 兜底（-21/-22 自动换票重放）。
ut 由本脚本计算（算法已用本机 2 条真实报文逐字节验证，见 tools/test_readaloud.py）。

用法：python tools/readaloud_post.py --row work/readaloud-prep/row.json [--send]
"""
import argparse
import hashlib
import json
import os
import subprocess
import sys
import time

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
URL = "https://study-api.up366.cn/client/task/score/submit/v2"
SALT = "submitTaskToken-pc-987654"


def md5(s):
    return hashlib.md5(s.encode("utf-8")).hexdigest()


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--row", default="work/readaloud-prep/row.json")
    ap.add_argument("--send", action="store_true")
    args = ap.parse_args()

    row = json.load(open(args.row, encoding="utf-8"))

    # 客户端 uploadScore 的两处改写：qstJson 对象→数组；extendParams 字符串→对象
    q = row["qstJson"]
    row["qstJson"] = list(json.loads(q).values()) if isinstance(q, str) else q
    e = row.get("extendParams")
    if isinstance(e, str):
        row["extendParams"] = json.loads(e) if e.strip() else {}

    tasks_json = json.dumps([row], ensure_ascii=False)
    r = md5(str(int(time.time() * 1000)))
    ut = r[:10] + md5(md5(tasks_json) + SALT + r) + r[10:]

    print("URL        %s" % URL)
    print("tasksJson  %d 字符" % len(tasks_json))
    print("qstJson    %d 条" % len(row["qstJson"]))
    print("score      %s / percent %s" % (row["score"], row["percent"]))
    print("ut         %s (%d)" % (ut, len(ut)))

    if not args.send:
        print("\n[dry-run] 未发送。加 --send。")
        return

    js = """
(async () => {
  const body = { tasksJson: %s, submitType: %s, ut: %s };
  try {
    const r = await window.AppInfo.httpRequest('post', %s, body);
    return JSON.stringify(r);
  } catch (err) {
    return JSON.stringify({ err: String(err && err.message || err) });
  }
})()
""" % (json.dumps(tasks_json), json.dumps(str(row.get("submitType", 8))), json.dumps(ut), json.dumps(URL))

    env = dict(os.environ)
    env["MSYS_NO_PATH_CONV"] = "1"
    env["U3_FRAME"] = "=local://res/index.html#/"
    p = subprocess.run(["node", os.path.join(REPO, "harness", "u3.js"), "eval", js],
                       capture_output=True, text=True, env=env, timeout=180)
    out = (p.stdout or "").strip()
    print("\n[response] " + out[:800])
    if p.returncode != 0:
        sys.exit("发送失败: " + (p.stderr or "").strip()[:300])

    with open(os.path.join(os.path.dirname(args.row), "response.json"), "w", encoding="utf-8") as f:
        f.write(out)


if __name__ == "__main__":
    main()
