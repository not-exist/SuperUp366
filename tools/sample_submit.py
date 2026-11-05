#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""单词PK 提交接口样例（协议复现用）。

链路（全部来自 recon/feature-word-pk.md 的逆向结论）：
  登录   POST https://sso.up366.cn/v2/tickets-v3
         password = md5(md5(明文) + "fa&s*l%$k!fq$k!ld@fjlk")
         ut = md5(ts)[:10] + md5(user+password+"loginEncryptionParameter"+md5(ts)) + md5(ts)[10:]
         -> data.tgt  即 u3t 头
  票据   业务接口返回 -21/-22 -> POST https://sso.up366.cn/v1/serviceTicket {service:<目标URL>}
         -> code -24 且 data.serverTicket -> 原请求追加 ?ticket=<ST> 重放一次
  词库   POST https://words-v2-api.up366.cn/client/sync/teaching/bucket/detail-info
         {source:1, bucketId:<pkBucketId>}  -> data.contentList[].entryList[]
  提交   GET  https://wordsbtl-api.up366.cn/client/wordsbtl/student/submit          (任务态)
         POST https://wordsbtl-api.up366.cn/client/wordsbtl/student/word-king/submit (词王争霸/活动)

默认 dry-run，只打印不发；加 --send 才真的写记录。
"""
import argparse
import hashlib
import http.cookiejar
import json
import random
import sys
import time
import urllib.error
import urllib.parse
import urllib.request
import uuid

LOGIN_URL = "https://sso.up366.cn/v2/tickets-v3"
ST_URL = "https://sso.up366.cn/v1/serviceTicket"
BUCKET_URL = "https://words-v2-api.up366.cn/client/sync/teaching/bucket/detail-info"
HOST = "https://wordsbtl-api.up366.cn"
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
        resp = json.loads(r.read().decode("utf-8", "replace"))
    return resp


def get_service_ticket(service, headers):
    body = urllib.parse.urlencode({"service": service}).encode()
    req = urllib.request.Request(ST_URL, data=body, method="POST")
    for k, v in headers.items():
        req.add_header(k, v)
    req.add_header("content-type", "application/x-www-form-urlencoded")
    with OPENER.open(req, timeout=20) as r:
        return json.loads(r.read().decode("utf-8", "replace"))


def raw_request(url, method, params, headers):
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
            return r.status, r.read().decode("utf-8", "replace")
    except urllib.error.HTTPError as e:
        return e.code, e.read().decode("utf-8", "replace")


def call(url, method, params, headers, service_url=None):
    """发一次；遇到 -21/-22 自动换 ST 重放一次（与客户端 M2.post/get 逻辑一致）。"""
    status, text = raw_request(url, method, params, headers)
    try:
        code = (json.loads(text).get("result") or {}).get("code")
    except Exception:
        code = None
    if code in (-21, -22):
        for svc in (service_url or url, url):
            st = get_service_ticket(svc, headers)
            stc = (st.get("result") or {}).get("code")
            ticket = (st.get("data") or {}).get("serverTicket")
            print("[st] service=%s -> code=%s" % (svc[:90], stc))
            if stc == -24 and ticket:
                sep = "&" if ("?" in url or method == "GET" and params) else "?"
                base = url + ("?" + urllib.parse.urlencode(params) if (method == "GET" and params) else "")
                retry_url = base + ("&" if "?" in base else "?") + "ticket=" + urllib.parse.quote(ticket)
                if method == "POST":
                    body = urllib.parse.urlencode(params).encode()
                    req = urllib.request.Request(retry_url, data=body, method="POST")
                    req.add_header("content-type", "application/x-www-form-urlencoded")
                else:
                    req = urllib.request.Request(retry_url, method="GET")
                for k, v in headers.items():
                    req.add_header(k, v)
                try:
                    with OPENER.open(req, timeout=25) as r:
                        return r.status, r.read().decode("utf-8", "replace")
                except urllib.error.HTTPError as e:
                    return e.code, e.read().decode("utf-8", "replace")
    return status, text


def fetch_bucket_words(bucket_id, headers):
    status, text = call(BUCKET_URL, "POST", {"source": 1, "bucketId": bucket_id}, headers)
    j = json.loads(text)
    if (j.get("result") or {}).get("code") != 0:
        raise RuntimeError("bucket 返回异常: " + text[:300])
    words = []
    for group in (j.get("data") or {}).get("contentList") or []:
        for e in group.get("entryList") or []:
            words.append(e)
    return words


def build_word_infos(words, count, right, seed=None, fields=None):
    """从词库随机抽 count 个词：right 个标记答对，其余答错。

    只保留 fields 指定的字段（任务态走 GET，整包要塞进 query，字段太多会 414）。
    """
    rnd = random.Random(seed)
    picked = rnd.sample(words, count) if count <= len(words) else list(words)
    out = []
    now = int(time.time() * 1000)
    order = list(range(len(picked)))
    rnd.shuffle(order)
    right_set = set(order[:right])
    for i, w in enumerate(picked):
        rec = {}
        for f in (fields or []):
            if f == "answer":
                rec["answer"] = ""
            elif f == "answerStatus":
                rec["answerStatus"] = 1 if i in right_set else 0
            elif f == "answerTime":
                rec["answerTime"] = now + i * 1200
            elif f in w:
                rec[f] = w[f]
        out.append(rec)
    return out


def main():
    ap = argparse.ArgumentParser(description="单词PK 提交接口样例")
    ap.add_argument("--branch", choices=["task", "arena"], default="task")
    ap.add_argument("--login", default="", help="账号（配合 --password 自动登录取 TGT）")
    ap.add_argument("--password", default="")
    ap.add_argument("--client-id", required=True, help="设备 MachineCode")
    ap.add_argument("--course-id", required=True)
    ap.add_argument("--study-task-id", default="")
    ap.add_argument("--role", default="Red", choices=["Red", "Blue"])
    ap.add_argument("--total", type=int, default=0, help="题数；不给则默认 = 分数+1")
    ap.add_argument("--right", type=int, default=0, help="分数（rightNum/score）")
    ap.add_argument("--duration", default="")
    ap.add_argument("--word-infos", default="", help="逐词明细 JSON 数组字符串")
    ap.add_argument("--bucket-id", default="", help="词库 bucketId；给了才能 --draw")
    ap.add_argument("--draw", type=int, default=0, help="从词库随机抽 N 个词作为答题明细")
    ap.add_argument("--seed", type=int, default=None, help="随机种子（便于复现）")
    ap.add_argument("--word-fields", default="entryId,answer,answerStatus,answerTime",
                    help="wordInfos 里保留的字段（逗号分隔）；支持 answer/answerStatus/answerTime 特殊键，字段越多 query 越长")
    ap.add_argument("--pk-type", default="单词PK")
    ap.add_argument("--pk-type-mode", default="limit", choices=["limit", "score"])
    ap.add_argument("--words-model", type=int, default=1)
    ap.add_argument("--invite-id", default="")
    ap.add_argument("--arena-type", type=int, default=1)
    ap.add_argument("--win", type=int, default=1)
    ap.add_argument("--token-id", default="")
    ap.add_argument("--replay", default="", help="客户端导出的成绩 JSON，回放真实成绩")
    ap.add_argument("--send", action="store_true", help="真的发出去；不给就是 dry-run")
    args = ap.parse_args()

    tgt = ""
    if args.login and args.password:
        resp = do_login(args.login, args.password, args.client_id)
        r0 = resp.get("result") or {}
        tgt = (resp.get("data") or {}).get("tgt")
        print("[login] code=%s msg=%s" % (r0.get("code"), r0.get("msg")))
        if not tgt:
            print(json.dumps(resp, ensure_ascii=False)[:600])
            sys.exit(1)
        print("[login] TGT 长度 %d" % len(tgt))
    headers = build_headers(args.client_id, tgt)

    word_infos = []
    if args.replay:
        raw = json.load(open(args.replay, encoding="utf-8"))

        def pick(v):
            if v is None:
                return None
            if isinstance(v, (dict, list)):
                return v
            try:
                return json.loads(v)
            except Exception:
                return None

        score = pick(raw.get("score")) or {}
        word_infos = pick(raw.get("data")) or []
        args.right = int(score.get("rightNum", score.get("score", 0)) or 0)
        args.total = int(score.get("totalNum", len(word_infos)) or 0)
        args.duration = score.get("duration", "")
        print("[replay] total=%s right=%s words=%d" % (args.total, args.right, len(word_infos)))
    elif args.draw:
        if not args.bucket_id:
            sys.exit("--draw 需要 --bucket-id")
        words = fetch_bucket_words(args.bucket_id, headers)
        print("[bucket] 词库共 %d 个词" % len(words))
        count = args.draw
        fields = [f.strip() for f in args.word_fields.split(",") if f.strip()]
        word_infos = build_word_infos(words, count, min(args.right, count), args.seed, fields)
        args.right = min(args.right, count)
        args.total = count
        print("[draw] 抽 %d 个词，答对 %d，答错 %d" % (len(word_infos), args.right, len(word_infos) - args.right))
    elif args.word_infos:
        word_infos = json.loads(args.word_infos)

    if args.total <= 0:
        args.total = args.right + 1
    if args.right <= 0 and not word_infos:
        sys.exit("需要 --right 分数，或 --draw/--replay 提供答题明细")

    if args.branch == "task":
        if not args.study_task_id:
            sys.exit("task 分支需要 --study-task-id")
        url = HOST + "/client/wordsbtl/student/submit"
        submit_json = {
            "totalNum": args.total,
            "rightNum": args.right,
            "score": args.right,
            "wordInfos": word_infos,
            "duration": args.duration,
        }
        params = {
            "courseId": args.course_id,
            "studyTaskId": args.study_task_id,
            "roleName": args.role,
            "roleType": "0",
            "submitJson": json.dumps(submit_json, separators=(",", ":"), ensure_ascii=False),
            "wrongLength": max(0, args.total - args.right),
            "score": args.right,
        }
        method = "GET"
    else:
        url = HOST + "/client/wordsbtl/student/word-king/submit"
        submit_json = {
            "gameModel": args.pk_type,
            "playModel": 1 if args.pk_type_mode == "score" else 0,
            "wordsModel": args.words_model,
            "totalNum": args.total,
            "rightNum": args.right,
            "score": args.right,
            "wordInfos": word_infos,
            "duration": args.duration,
        }
        params = {
            "courseId": args.course_id,
            "submitJson": json.dumps(submit_json, separators=(",", ":"), ensure_ascii=False),
            "inviteId": args.invite_id,
            "type": args.arena_type,
            "worldWinFlag": args.win,
            "tokenId": args.token_id,
        }
        method = "POST"

    projected = len(url) + len(urllib.parse.urlencode(params))
    print("[size] 预计 URL 长度 %d 字节（GET 提交建议 < 7000，超过会被 ALB 挡掉）" % projected)
    if projected > 7000:
        print("[warn] 超过 7000，很可能 400/414；用 --word-fields 精简，例如 entryId,answerStatus")
    print("=== 请求 ===")
    print(method, url)
    for k, v in headers.items():
        print("  %-16s %s" % (k, (v[:8] + "...") if k == "u3t" else v))
    print("--- params ---")
    for k, v in params.items():
        v = str(v)
        print("  %-16s %s" % (k, v if len(v) <= 150 else v[:150] + "...(%d)" % len(v)))

    if not args.send:
        print("")
        print("[dry-run] 未发送。确认无误后加 --send。")
        return

    status, text = call(url, method, params, headers)
    print("")
    print("[HTTP %s]" % status)
    print(text[:2000])


if __name__ == "__main__":
    main()
