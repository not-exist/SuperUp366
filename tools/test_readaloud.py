# -*- coding: utf-8 -*-
"""核心语义测试：ut 构造与真实历史报文一致。

为什么需要这个：`ut` 是提交必带字段，算错就整包被拒（result.code != 0）。
本地 `commonupload` 表里存着两条**真实**课本提交报文（含 ut），可直接当 oracle。

跑法：python tools/test_readaloud.py
"""
import hashlib
import json
import os
import sys

DUMP = os.path.join(os.path.dirname(os.path.abspath(__file__)), "fixtures", "real-submit-payloads.json")


def md5(s):
    return hashlib.md5(s.encode("utf-8")).hexdigest()


def build_ut(payload, ts_ms, salt="submitTaskToken-pc-987654"):
    """与客户端 buildUT 同构（recon/readaloud-complete.md §4.6）。"""
    r = md5(str(ts_ms))
    return r[:10] + md5(md5(payload) + salt + r) + r[10:]


def ut_from_real(ut):
    """从真实 ut 里还原 r（ut = r[:10] + 32位 + r[10:]）。"""
    return ut[:10] + ut[42:]


def main():
    if not os.path.exists(DUMP):
        sys.exit("缺少真实报文快照：%s" % DUMP)

    lines = [l for l in open(DUMP, encoding="utf-8").read().split("\n") if l.strip()]
    checked = 0
    for line in lines[1:]:
        try:
            row = json.loads(line)
        except Exception:
            continue
        for item in row.get("requestParams") or []:
            body = json.loads(item["body"])
            ut, tasks_json = body.get("ut"), body.get("tasksJson")
            if not ut or not tasks_json:
                continue
            assert len(ut) == 64, "ut 长度应为 64，实际 %d" % len(ut)
            r = ut_from_real(ut)
            # 两种盐写法必须给出同一结果（客户端内联版 vs buildUT 版）
            a = md5(md5(tasks_json) + "submitTaskToken-pc-987654" + r)
            c = md5(md5(tasks_json) + "submitTaskToken" + "-pc-987654" + r)
            assert a == c, "两种盐写法应等价"
            assert ut[10:42] == a, "ut 中段与真实报文不符"
            # 反向：用同一 r 重建 ut，必须逐字节相同（时间戳取自 ut 自身）
            rebuilt = r[:10] + a + r[10:]
            assert rebuilt == ut, "重建的 ut 应与真实报文完全一致"
            checked += 1
    assert checked >= 1, "没有可校验的真实报文"
    print("ok: %d 条真实报文的 ut 全部与构造算法一致（长度 64 / 中段 32 hex / 两种盐等价）" % checked)


if __name__ == "__main__":
    main()
