# -*- coding: utf-8 -*-
"""查课程作业列表，定位「Unit 4 课文1 逐句跟读」这条作业的 id。

提交行需要三个 id（不能凭空构造）：
  studyTaskId —— 作业任务 id
  taskId / chapterId —— 内容 id（客户端与真实报文里两者相同）
  paperId —— 试卷 id

来源接口（recon/hw-dub-2026-07-31.md §1）：
  POST https://studytask-api.up366.cn/client/student/course/job  {courseId}
  → data.allTaskList[]

用法：
  python tools/readaloud_job.py --login <账号> --password <口令> --client-id <id> --course-id 9222783
"""
import argparse
import json
import os
import sys
import time
import urllib.parse
import urllib.request
import uuid

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from readaloud_upload import build_headers, form_encode, md5, post_form  # noqa: E402

LOGIN_URL = "https://sso.up366.cn/v2/tickets-v3"
ST_URL = "https://sso.up366.cn/v1/serviceTicket"
JOB_URL = "https://studytask-api.up366.cn/client/student/course/job"
PWD_SALT = "fa&s*l%$k!fq$k!ld:fjlk"


def do_login(username, password, client_id):
    pwd = md5(md5(password) + PWD_SALT)
    ts = md5(str(int(time.time() * 1000)))
    ut = ts[:10] + md5(username + pwd + "loginEncryptionParameter" + ts) + ts[10:]
    resp = post_form(LOGIN_URL, form_encode({"username": username, "password": pwd, "ut": ut}),
                     build_headers(client_id, ""))
    tgt = (resp.get("data") or {}).get("tgt")
    print("[login] code=%s tgt=%s" % ((resp.get("result") or {}).get("code"),
                                      ("len=%d" % len(tgt)) if tgt else "-"))
    if not tgt:
        print(json.dumps(resp, ensure_ascii=False)[:400])
        sys.exit(1)
    return tgt


def call_with_st(url, body, headers):
    """发一次；遇 -21/-22 换 ST 重放一次（与客户端 M2.post 一致）。"""
    resp = post_form(url, body, headers)
    code = (resp.get("result") or {}).get("code")
    if code in (-21, -22):
        st = post_form(ST_URL, form_encode({"service": url}), headers)
        if (st.get("result") or {}).get("code") == -24:
            ticket = (st.get("data") or {}).get("serverTicket")
            sep = "&" if "?" in url else "?"
            print("[st] 换票后重放")
            resp = post_form(url + sep + "ticket=" + urllib.parse.quote(ticket), body, headers)
    return resp


def main():
    ap = argparse.ArgumentParser(description="查作业列表，定位逐句跟读作业 id")
    ap.add_argument("--login", default="")
    ap.add_argument("--password", default="")
    ap.add_argument("--client-id", required=True)
    ap.add_argument("--course-id", type=int, default=9222783)
    ap.add_argument("--keyword", default="Unit 4", help="按关键词过滤作业名")
    ap.add_argument("--out", default="")
    args = ap.parse_args()

    if not (args.login and args.password):
        sys.exit("需要 --login --password")
    tgt = do_login(args.login, args.password, args.client_id)
    headers = build_headers(args.client_id, tgt)

    resp = call_with_st(JOB_URL, form_encode({"courseId": args.course_id}), headers)
    code = (resp.get("result") or {}).get("code")
    print("[job] code=%s" % code)
    if code != 0:
        print(json.dumps(resp, ensure_ascii=False)[:600])
        sys.exit(1)

    tasks = (resp.get("data") or {}).get("allTaskList") or []
    print("[job] 共 %d 条作业" % len(tasks))
    hits = []
    for t in tasks:
        name = t.get("jobName") or t.get("name") or ""
        if args.keyword and args.keyword not in name:
            continue
        hits.append(t)
        print("")
        print("  jobName    : %s" % name)
        for k in ("jobId", "type", "courseId", "contentId", "studyTaskId", "studyTaskPid",
                  "assignId", "assignItemId", "beginTime", "endTime"):
            if k in t:
                print("  %-12s: %s" % (k, t[k]))
        ext = t.get("extParam")
        if isinstance(ext, str):
            try:
                ext = json.loads(ext)
            except Exception:
                pass
        if isinstance(ext, dict):
            for k in ("bookId", "chapterId", "bookName", "taskType", "taskScore", "bookType"):
                if k in ext:
                    print("  ext.%-9s: %s" % (k, ext[k]))
            xp = ext.get("xotParam")
            if xp:
                print("  ext.xotParam: %s" % xp)

    if args.out:
        with open(args.out, "w", encoding="utf-8") as f:
            json.dump({"courseId": args.course_id, "hits": hits}, f, ensure_ascii=False, indent=2)
        print("\nwritten %s" % args.out)


if __name__ == "__main__":
    main()
