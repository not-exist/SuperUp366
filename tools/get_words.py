#!/usr/bin/env python3
# -*- coding: utf-8 -*-
r"""获取「单词PK」的真实词表。

两个来源，按优先级：

  1. startGame 的 data.words
     GET https://wordsbtl-api.up366.cn/client/wordsbtl/student/start
         ?courseId=&studyTaskId=&taskType=0
     只有对局处于进行中（双方都没提交）才会带 words；已有一方提交就只回 group。

  2. 词库 bucket detail-info（可靠来源）
     POST https://words-v2-api.up366.cn/client/sync/teaching/bucket/detail-info
         {source:1, bucketId:<pkBucketId>}
     -> data.contentList[].entryList[]
     实测与任务的 xotParam.wordNum 完全一致：
       10.30词汇PK/单词PK   wordNum=448  bucket=448
       7.1单词大PK(Unit4)   wordNum=137  bucket=137
     即：bucket 就是这场 PK 的真实词表。

pkBucketId 从哪来：
  * 客户端日志 C:\Up366StudentFiles\logs\app.log 里搜 downloadResource，
    extParams.xotParam.pkBucketId 就是；
  * 或作业详情接口（H5 下发的 extParam.xotParam）。

用法：
  python tools/get_words.py --login <账号> --password <口令> \
      --client-id 7057119429F1E0F7138F82EFF5BFE50D270DD095 \
      --course-id 9222783 --study-task-id 3ACABB8F554F5E9A746EF1E7D8C7511B \
      --bucket-id 6D1BB3B35BD73C6FADEDA62C8F197F62 \
      --out work/words/3ACABB8F.json --format json
"""
import argparse
import hashlib
import http.cookiejar
import json
import os
import sys
import time
import urllib.error
import urllib.parse
import urllib.request
import uuid

LOGIN_URL = "https://sso.up366.cn/v2/tickets-v3"
ST_URL = "https://sso.up366.cn/v1/serviceTicket"
START_URL = "https://wordsbtl-api.up366.cn/client/wordsbtl/student/start"
WORDS_INFO_URL = "https://wordsbtl-api.up366.cn/client/wordsbtl/student/words/info"
BUCKET_URL = "https://words-v2-api.up366.cn/client/sync/teaching/bucket/detail-info"
PWD_SALT = "fa&s*l%$k!fq$k!ld@fjlk"

JAR = http.cookiejar.CookieJar()
OPENER = urllib.request.build_opener(urllib.request.HTTPCookieProcessor(JAR))


def md5(s):
    return hashlib.md5(s.encode("utf-8")).hexdigest()


def build_headers(client_id, tgt):
    return {
        "user-agent": "PC-Up366-Student 6.11.0",
        "x-requested-with": "PC",
        "X-App-Name": "student-pc",
        "clientId": client_id,
        "u3r": str(uuid.uuid4()),
        "u3t": tgt,
        "Authorization": "",
    }


def do_login(username, password, client_id):
    pwd = md5(md5(password) + PWD_SALT)
    ts = md5(str(int(time.time() * 1000)))
    ut = ts[:10] + md5(username + pwd + "loginEncryptionParameter" + ts) + ts[10:]
    body = urllib.parse.urlencode({"username": username, "password": pwd, "ut": ut}).encode()
    req = urllib.request.Request(LOGIN_URL, data=body, method="POST")
    req.add_header("content-type", "application/x-www-form-urlencoded")
    for k, v in build_headers(client_id, "").items():
        req.add_header(k, v)
    with OPENER.open(req, timeout=20) as r:
        return json.loads(r.read().decode("utf-8", "replace"))


def get_service_ticket(service, headers):
    body = urllib.parse.urlencode({"service": service}).encode()
    req = urllib.request.Request(ST_URL, data=body, method="POST")
    for k, v in headers.items():
        req.add_header(k, v)
    req.add_header("content-type", "application/x-www-form-urlencoded")
    with OPENER.open(req, timeout=20) as r:
        return json.loads(r.read().decode("utf-8", "replace"))


def call(url, method, params, headers):
    if method == "GET":
        target = url + ("?" + urllib.parse.urlencode(params) if params else "")
        req = urllib.request.Request(target, method="GET")
    else:
        target = url
        req = urllib.request.Request(url, data=urllib.parse.urlencode(params).encode(), method="POST")
        req.add_header("content-type", "application/x-www-form-urlencoded")
    for k, v in headers.items():
        req.add_header(k, v)
    try:
        with OPENER.open(req, timeout=25) as r:
            status, text = r.status, r.read().decode("utf-8", "replace")
    except urllib.error.HTTPError as e:
        status, text = e.code, e.read().decode("utf-8", "replace")

    try:
        code = (json.loads(text).get("result") or {}).get("code")
    except Exception:
        code = None
    if code in (-21, -22):
        st = get_service_ticket(url, headers)
        stc = (st.get("result") or {}).get("code")
        ticket = (st.get("data") or {}).get("serverTicket")
        if stc == -24 and ticket:
            sep = "&" if "?" in target else "?"
            req2 = urllib.request.Request(target + sep + "ticket=" + urllib.parse.quote(ticket), method=method)
            for k, v in headers.items():
                req2.add_header(k, v)
            if method != "GET":
                req2.add_header("content-type", "application/x-www-form-urlencoded")
            try:
                with OPENER.open(req2, timeout=25) as r:
                    status, text = r.status, r.read().decode("utf-8", "replace")
            except urllib.error.HTTPError as e:
                status, text = e.code, e.read().decode("utf-8", "replace")
    return json.loads(text)


def words_from_start(course_id, task_id, task_type, headers):
    """对局进行中时，startGame 会带回本局出题顺序。"""
    j = call(START_URL, "GET", {"courseId": course_id, "studyTaskId": task_id, "taskType": task_type}, headers)
    data = j.get("data") or {}
    words = data.get("words") or []
    group = data.get("group") or []
    return words, "startGame(words=%d, group=%d, taskType=%s)" % (len(words), len(group), task_type)


def words_from_bucket(bucket_id, headers):
    j = call(BUCKET_URL, "POST", {"source": 1, "bucketId": bucket_id}, headers)
    if (j.get("result") or {}).get("code") != 0:
        raise RuntimeError("bucket 返回异常: " + json.dumps(j, ensure_ascii=False)[:300])
    words = []
    for group in (j.get("data") or {}).get("contentList") or []:
        for e in group.get("entryList") or []:
            words.append(e)
    n_groups = len(((j.get("data") or {}).get("contentList")) or [])
    return words, "bucket(bucketId=%s, contentList 分组 %d 个)" % (bucket_id, n_groups)


def flatten(words, keep):
    out = []
    for w in words:
        rec = {}
        for f in keep:
            if f in w:
                rec[f] = w[f]
        out.append(rec)
    return out


def main():
    ap = argparse.ArgumentParser(description="获取单词PK真实词表")
    ap.add_argument("--login", default="", help="账号（配 --password 自动登录取 TGT）")
    ap.add_argument("--password", default="")
    ap.add_argument("--client-id", required=True)
    ap.add_argument("--course-id", required=True)
    ap.add_argument("--study-task-id", required=True)
    ap.add_argument("--bucket-id", default="", help="pkBucketId；startGame 拿不到词表时用")
    ap.add_argument("--task-type", type=int, default=0)
    ap.add_argument("--format", choices=["json", "csv", "txt"], default="json")
    ap.add_argument("--fields", default="entryId,pkid,entry,paraphrase,enPhonetic,usPhonetic",
                    help="json/csv 输出里保留的词库字段")
    ap.add_argument("--out", default="", help="输出文件；不给就只打印")
    a = ap.parse_args()

    if not (a.login and a.password):
        sys.exit("需要 --login/--password（客户端里就是 AppInfo.getUserLoginInfo() 那对）")

    resp = do_login(a.login, a.password, a.client_id)
    r0 = resp.get("result") or {}
    tgt = (resp.get("data") or {}).get("tgt")
    print("[login] code=%s msg=%s" % (r0.get("code"), r0.get("msg")))
    if not tgt:
        print(json.dumps(resp, ensure_ascii=False)[:500])
        sys.exit(1)
    headers = build_headers(a.client_id, tgt)

    words, source = words_from_start(a.course_id, a.study_task_id, a.task_type, headers)
    print("[startGame] words=%d" % len(words))

    if not words:
        if not a.bucket_id:
            sys.exit("startGame 没带词表（对局已结束/未开始）。\n"
                     "解决：把 pkBucketId 用 --bucket-id 传进来。\n"
                     "找法：grep downloadResource C:/Up366StudentFiles/logs/app.log，"
                     "extParams.xotParam.pkBucketId 就是。")
        words, source = words_from_bucket(a.bucket_id, headers)
        print("[bucket] words=%d" % len(words))

    keep = [f.strip() for f in a.fields.split(",") if f.strip()]
    flat = flatten(words, keep)
    print("[词表] 来源=%s 共 %d 词" % (source, len(flat)))
    for w in flat[:5]:
        print("   ", json.dumps(w, ensure_ascii=False)[:160])
    if len(flat) > 5:
        print("    ...")

    if not a.out:
        return

    os.makedirs(os.path.dirname(os.path.abspath(a.out)), exist_ok=True)
    if a.format == "json":
        json.dump(flat, open(a.out, "w", encoding="utf-8"), ensure_ascii=False, indent=1)
    elif a.format == "csv":
        import csv
        cols = keep or sorted(flat[0].keys())
        with open(a.out, "w", encoding="utf-8-sig", newline="") as f:
            wr = csv.DictWriter(f, fieldnames=cols, extrasaction="ignore")
            wr.writeheader()
            for w in flat:
                wr.writerow(w)
    else:
        with open(a.out, "w", encoding="utf-8") as f:
            for w in flat:
                f.write(w.get("entry", "") + "\n")
    print("[out] 已写入 %s (%s)" % (a.out, a.format))


if __name__ == "__main__":
    main()
