# -*- coding: utf-8 -*-
"""核心语义测试：objectName 生成规则与客户端一致。

为什么需要这个：`objectName` 决定 OSS 上的对象键，也是 `objectIds`（提交报文里的附件清单）
的来源。规则错一位 → 服务端按 objectName 找不到音频 → 提交里分数正常但音频丢失。

oracle：客户端自己的公式（`buildOSSObjectName`，index.js@3556173），本文件里逐行复刻一份。

跑法：python tools/test_readaloud.py
"""
import datetime
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from readaloud_upload import EXT_CLASS, build_object_name  # noqa: E402


def client_build_oss_object_name(scope, filename, cloud, user_type, uid, date_str, ext_class_map):
    """客户端 buildOSSObjectName 的直译（index.js@3556173），作为 oracle。"""
    def ext_class(name):
        c = name[name.rfind(".") + 1:].lower()
        return ext_class_map.get(c, "unknown")

    n = s = ""
    r = cloud
    if user_type:
        s = "/%s" % user_type
    if uid:
        n = "/%s" % uid
    if r:
        r = "/%s" % r
    return "storage%s/%s/%s/%s%s%s/%s" % (s, scope, ext_class(filename), date_str, n, r, filename)


CASES = [
    # (user_type, uid, cloud, filename, 期望日期串)
    ("", 39370562, "", "12edb6ff1d2c424db90778a256d06ec4.mp3", "2026/11/03"),
    ("1", 39370562, "", "abc.mp3", "2026/11/03"),
    ("", 0, "", "abc.mp3", "2026/11/03"),
    ("1", 39370562, "minicloud", "abc.mp3", "2026/11/03"),
    ("", 39370562, "", "noext", "2026/11/03"),
]


def main():
    checked = 0
    for user_type, uid, cloud, filename, date_str in CASES:
        expect = client_build_oss_object_name("study", filename, cloud, user_type, uid, date_str, EXT_CLASS)
        # 我们的实现用 datetime.now()，测试里固定日期以对齐 oracle
        when = datetime.datetime(2026, 11, 3, 12, 0, 0)
        got = build_object_name(user_type, str(uid) if uid else "", filename, cloud, when=when)
        assert got == expect, "objectName 不一致:\n  客户端 %s\n  本实现 %s" % (expect, got)
        checked += 1

    # 扩展名映射必须覆盖客户端 $At 表里的全部音频类型
    for ext in ("mp3", "wav", "aac", "flac", "ogg", "wma", "m4a", "opus", "amr"):
        assert EXT_CLASS.get(ext) == "a", "扩展名 %s 的类别应为 a" % ext
    checked += 1
    # 未知扩展名由调用处兜底成 "unknown"（客户端用的是 ?? "unknown"）
    assert EXT_CLASS.get("xyz") is None, "未知扩展名不应出现在映射表里"
    got_unknown = build_object_name("", "1", "file.xyz", "", when=datetime.datetime(2026, 11, 3))
    assert "/unknown/" in got_unknown, "未知扩展名应落到 unknown 目录：%s" % got_unknown

    print("ok: %d 个用例的 objectName 与客户端公式逐字符一致（扩展名映射覆盖 9 种音频格式）" % checked)


if __name__ == "__main__":
    main()
