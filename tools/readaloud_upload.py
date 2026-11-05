# -*- coding: utf-8 -*-
"""逐句跟读音频上传（OSS 直传）。

链路（与 recon/readaloud-complete.md §3.2 / §4.6 核对过）：
  1) POST sso /v2/tickets-v3              -> TGT（后续作 u3t 头）
  2) GET  setup-api /front/aliyunoss/token/v2?organId=<schoolId> -> STS
  3) objectName 按客户端 buildOSSObjectName 规则本地生成：
       storage{userType}/study/a/{yyyy/mm/dd}/{uid}[/{cloud}]/{fileName}
  4) Aliyun OSS V1 签名 PUT 上传 mp3
  5) 回填 https://{bucketname}.{defaulthostid}/{objectName} 到 task.json

默认 dry-run（只打印将要上传的对象名与 URL）；加 --send 才真的上传。

用法：
  python tools/readaloud_upload.py --task work/readaloud-prep/task.json \
      --mp3 q1=q1.mp3,q2=q2.mp3,q3=q3.mp3 \
      --login <账号> --password <口令> --client-id <MachineCode> --school-id <学校id> --uid <uid>
"""
import argparse
import base64
import datetime
import hashlib
import hmac
import json
import os
import sys
import urllib.parse
import urllib.request
import uuid

LOGIN_URL = "https://sso.up366.cn/v2/tickets-v3"
ST_URL = "https://sso.up366.cn/v1/serviceTicket"
STS_URL = "https://setup-api.up366.cn/front/aliyunoss/token/v2"
PWD_SALT = "fa&s*l%$k!fq$k!ld:fjlk"

# 扩展名 → objectName 里的目录类别（客户端 $At 表，index.js@3553352）
EXT_CLASS = {
    "mp3": "a", "wav": "a", "aac": "a", "flac": "a", "ogg": "a", "wma": "a",
    "m4a": "a", "opus": "a", "amr": "a",
}


def md5(s):
    return hashlib.md5(s.encode("utf-8")).hexdigest()


def build_headers(client_id, tgt):
    return {
        "user-agent": "PC-Up366-Student 6.11.0",
        "x-requested-with": "PC",
        "X-App-Name": "student-pc",
        "clientId": client_id,
        "u3r": uuid.uuid4().hex,
        "u3t": tgt or "",
        "Authorization": "",
    }


def form_encode(params):
    out = {}
    for k, v in params.items():
        out[k.replace("###", ".")] = json.dumps(v, ensure_ascii=False) if isinstance(v, (dict, list)) else str(v)
    return urllib.parse.urlencode(out).encode()


def post_form(url, body, headers):
    req = urllib.request.Request(url, data=body, method="POST")
    req.add_header("content-type", "application/x-www-form-urlencoded")
    for k, v in headers.items():
        req.add_header(k, v)
    with urllib.request.urlopen(req, timeout=30) as r:
        return json.loads(r.read().decode("utf-8", "replace"))


def get_json(url, headers):
    req = urllib.request.Request(url, method="GET")
    for k, v in headers.items():
        req.add_header(k, v)
    with urllib.request.urlopen(req, timeout=30) as r:
        return json.loads(r.read().decode("utf-8", "replace"))


def do_login(username, password, client_id):
    pwd = md5(md5(password) + PWD_SALT)
    ts = md5(str(int(datetime.datetime.now().timestamp() * 1000)))
    ut = ts[:10] + md5(username + pwd + "loginEncryptionParameter" + ts) + ts[10:]
    body = form_encode({"username": username, "password": pwd, "ut": ut})
    resp = post_form(LOGIN_URL, body, build_headers(client_id, ""))
    code = (resp.get("result") or {}).get("code")
    tgt = (resp.get("data") or {}).get("tgt")
    print("[login] code=%s tgt=%s" % (code, ("len=%d" % len(tgt)) if tgt else "-"))
    if not tgt:
        print(json.dumps(resp, ensure_ascii=False)[:400])
        sys.exit(1)
    return tgt


def get_sts(school_id, client_id, tgt):
    """取 STS。data 可能是数组（每个元素一个 bucket）或对象。"""
    url = STS_URL + "?organId=" + urllib.parse.quote(str(school_id))
    resp = get_json(url, build_headers(client_id, tgt))
    if (resp.get("result") or {}).get("code") != 0:
        sys.exit("取 STS 失败：" + json.dumps(resp, ensure_ascii=False)[:400])
    data = resp.get("data")
    buckets = []
    if isinstance(data, list):
        buckets = data
    elif isinstance(data, dict):
        # 兼容 {biz: [...]} 或 {bucketId: {...}} 两种形态
        for v in data.values():
            if isinstance(v, list):
                buckets.extend(v)
            elif isinstance(v, dict) and "ossBucketInfo" in v:
                buckets.append(v)
    for b in buckets:
        info = b.get("ossBucketInfo")
        if info:
            return info
    sys.exit("STS 响应里没有 ossBucketInfo：" + json.dumps(data, ensure_ascii=False)[:400])


def build_object_name(user_type, uid, filename, cloud="", when=None):
    """复刻客户端 buildOSSObjectName(scope='study', filename, cloud)。

    客户端（index.js@3556173）：
      storage{userType}/{scope}/{extClass}/{yyyy/mm/dd}{uid}[/{cloud}]/{fileName}
    注意 uid 段前面没有多余的斜杠拼接问题：日期后直接接 /uid。
    """
    d = when or datetime.datetime.now()
    ext = filename.rsplit(".", 1)[-1].lower() if "." in filename else ""
    ext_class = EXT_CLASS.get(ext, "unknown")
    parts = ["storage"]
    if user_type:
        parts.append("/%s" % user_type)
    parts.append("/study")
    parts.append("/%s" % ext_class)
    parts.append("/%s" % d.strftime("%Y/%m/%d"))
    if uid:
        parts.append("/%s" % uid)
    if cloud:
        parts.append("/%s" % cloud)
    parts.append("/%s" % filename)
    return "".join(parts)


def oss_put(url, sts, object_name, data, content_type="audio/mpeg"):
    """Aliyun OSS V1 签名 PUT。

    Authorization: OSS <AccessKeyId>:<Signature>
    Signature = base64(hmac-sha1(AccessKeySecret, VERB\\nContent-MD5\\nContent-Type\\nDate\\nCanonicalizedResource))
    """
    p = urllib.parse.urlsplit(url)
    bucket, host = p.netloc.split(".")[0], p.netloc
    date = datetime.datetime.utcnow().strftime("%a, %d %b %Y %H:%M:%S GMT")
    resource = "/%s/%s" % (bucket, object_name)
    string_to_sign = "\n".join(["PUT", hashlib.md5(data).hexdigest(), content_type, date, resource])
    sig = base64.b64encode(
        hmac.new(sts["tmpAkSecret"].encode("utf-8"), string_to_sign.encode("utf-8"), hashlib.sha1).digest()
    ).decode()
    req = urllib.request.Request("https://%s/%s" % (host, object_name), data=data, method="PUT")
    req.add_header("Authorization", "OSS %s:%s" % (sts["tmpAkId"], sig))
    req.add_header("Date", date)
    req.add_header("Content-MD5", hashlib.md5(data).hexdigest())
    req.add_header("Content-Type", content_type)
    req.add_header("x-oss-security-token", sts["securityToken"])
    with urllib.request.urlopen(req, timeout=180) as r:
        if r.status != 200:
            raise RuntimeError("OSS PUT 返回 %s" % r.status)
    return "https://%s/%s" % (host, object_name)


def main():
    ap = argparse.ArgumentParser(description="上传逐句跟读音频到 OSS（默认 dry-run）")
    ap.add_argument("--task", default="work/readaloud-prep/task.json")
    ap.add_argument("--mp3", required=True, help="每题音频，格式 q1=a.mp3,q2=b.mp3,q3=c.mp3")
    ap.add_argument("--login", default="")
    ap.add_argument("--password", default="")
    ap.add_argument("--client-id", default="")
    ap.add_argument("--school-id", default="")
    ap.add_argument("--uid", default="")
    ap.add_argument("--user-type", default="", help="CurrentUser.type，影响 objectName 前缀")
    ap.add_argument("--cloud", default="", help="私有云时填 minicloud，否则留空")
    ap.add_argument("--send", action="store_true", help="真的上传；缺省只打印")
    args = ap.parse_args()

    task = json.load(open(args.task, encoding="utf-8"))
    questions = task["questions"]

    pairs = []
    for item in args.mp3.split(","):
        if not item.strip():
            continue
        k, v = item.split("=", 1)
        pairs.append((k.strip(), v.strip()))
    if len(pairs) != len(questions):
        sys.exit("--mp3 数量(%d)与题数(%d)不符" % (len(pairs), len(questions)))

    for (key, path), q in zip(pairs, questions):
        if not os.path.exists(path):
            sys.exit("找不到音频文件：%s" % path)
        if key not in ("q1", "q2", "q3") and key != str(q.get("index")):
            print("[warn] %s 与题目序号不匹配" % key)

    # 1) objectName 本地生成
    plans = []
    for (key, path), q in zip(pairs, questions):
        name = "%s.mp3" % uuid.uuid4().hex
        obj = build_object_name(args.user_type, args.uid, name, args.cloud)
        plans.append({"key": key, "path": path, "objectName": obj, "question": q["elementId"]})

    print("=== 计划上传 ===")
    for p in plans:
        print("  %s  %-10s  %8d B  %s" % (p["key"], p["question"], os.path.getsize(p["path"]), p["objectName"]))
    print("")
    print("[dry-run] 未上传。加 --send 并提供 --login/--password/--client-id/--school-id/--uid 才真的传。")
    if not args.send:
        return

    if not (args.login and args.password and args.client_id):
        sys.exit("--send 需要 --login --password --client-id")

    # 2) 登录取 TGT
    tgt = do_login(args.login, args.password, args.client_id)

    # 3) 取 STS
    sts = get_sts(args.school_id, args.client_id, tgt)
    print("[sts] bucket=%s endpoint=%s" % (sts.get("bucketname"), sts.get("defaulthostid")))
    host = "%s.%s" % (sts["bucketname"], sts["defaulthostid"])

    # 4) 上传
    urls = []
    for p in plans:
        data = open(p["path"], "rb").read()
        url = oss_put("https://%s/%s" % (host, p["objectName"]), sts, p["objectName"], data)
        p["url"] = url
        urls.append(url)
        print("[upload] %s -> %s" % (p["key"], url))

    # 5) 回填 task.json
    for q, p in zip(questions, plans):
        q["ossObjectName"] = p["objectName"]
        q["ossUrl"] = p["url"]
    task["ossUrls"] = urls
    with open(args.task, "w", encoding="utf-8") as f:
        json.dump(task, f, ensure_ascii=False, indent=2)
    print("updated %s" % args.task)


if __name__ == "__main__":
    main()
