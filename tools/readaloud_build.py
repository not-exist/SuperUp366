# -*- coding: utf-8 -*-
"""逐句跟读成绩报文构造器（只构造，不发送）。

按本地实测到的真实报文结构生成 tasksJson / ut / XML：
  - 分支 C（gzip）：POST /client/task/score/gzip/submit
      body = {tasksJson:"<JSON 字符串>", ut, submitType}
      行字段与 qstJson 形态见 recon/readaloud-complete.md §4.4 / §4.5
  - 分支 A（form）：POST /client/task/score/submit/v2
      body = {tasksJson:"<JSON 字符串>", submitType, ut}
      行字段与 qstJson 形态见 §4.2 / §4.5

XML 形状（实测）：
  answerdata = <elements> + 每题 <element id="题id"><answers>…</answers></element> + </elements>
  markdatao  = <elements> + 每题 <element id="题id">…<mark_data id="N">…</mark_data></element> + </elements>
  朗读题（question_type=9）的 <answers> 内：
      <answer_url>…</answer_url><answer_score>…</answer_score>
      <answer_rejectscore>0</answer_rejectscore><answer_text><![CDATA[…]]></answer_text>
      <answer_recordmin><![CDATA[…]]></answer_recordmin><answer_recordsec><![CDATA[…]]></answer_recordsec>

分数/音频由调用方给出；本工具不产生分数，只把它放进报文的对应位置。

用法：
  python tools/readaloud_build.py --task work/readaloud-prep/task.json \
      --scores 10,10,10 --urls "https://…/a.mp3,https://…/b.mp3,https://…/c.mp3" \
      --branch c --submit-type 8 --out work/readaloud-prep/body.json
"""
import argparse
import hashlib
import json
import os
import time
import uuid

# 客户端 buildOSSObjectName 用到的盐（见 recon/readaloud-complete.md §3.2）
SUBMIT_TOKEN_SALT = "submitTaskToken-pc-987654"


def md5(s):
    return hashlib.md5(s.encode("utf-8")).hexdigest()


def build_ut(payload, salt=SUBMIT_TOKEN_SALT):
    """ut 构造（客户端 submitTaskScore / buildUT 两种写法等价）。

    r = md5(str(Date.now())); ut = r[:10] + md5(md5(payload) + salt + r) + r[10:]
    实测长度 64。
    """
    r = md5(str(int(time.time() * 1000)))
    n = md5(md5(payload) + salt + r)
    return r[:10] + n + r[10:]


def esc_cdata(s):
    """CDATA 里的 ]]> 需要拆分，否则 XML 提前闭合。"""
    return str(s).replace("]]>", "]]]]><![CDATA[>")


def build_answer_element(q, url, score, record_min, record_sec, text):
    """朗读题 answerdata 的一个 <element>。"""
    return (
        '<element id="%s"><answers>'
        "<answer_url>%s</answer_url>"
        "<answer_score>%s</answer_score>"
        "<answer_rejectscore>0</answer_rejectscore>"
        "<answer_text><![CDATA[%s]]></answer_text>"
        "<answer_recordmin><![CDATA[%s]]></answer_recordmin>"
        "<answer_recordsec><![CDATA[%s]]></answer_recordsec>"
        "</answers></element>"
        % (q["elementId"], url, score, esc_cdata(text), record_min, record_sec)
    )


def build_mark_element(q, score, result, comment=""):
    """朗读题 markdatao 的一个 <element>（题级一条 mark_data，id 取题内子项数）。

    真实样本（填空题）里 mark_data id = 题内子项 order（1..N）；
    朗读题无历史样本，这里按题级单条（id 缺省）处理，与客户端 v() 的
    「无 markDataList 时只出一条 mark_data」分支一致。
    """
    return (
        '<element id="%s"><mark_data>'
        "<score>%s</score><result>%s</result>"
        "%s"
        "</mark_data></element>"
        % (q["elementId"], score, result,
           ("<comment><![CDATA[%s]]></comment>" % esc_cdata(comment)) if comment else "")
    )


def build_qst_json(questions, items):
    """qstJson 数组（分支 C 形态）。

    每题 1 条 elementType=1（题）+ N 条 elementType=2（题内子项，elementId = 题id-order）。
    elementAttr 字段名按实测全集。
    """
    out = []
    for q, it in zip(questions, items):
        el = q["elementId"]
        base = {
            "question_id": el,
            "question_type": 9,
            "order": 1,
            "user_score": it["score"],
            "user_answer": "",
            "answer": "",
            "score": q["score"],
            "result": it["result"],
            "timestamp": it["timestamp"],
            "record_url": it["url"],
            "duration": it["duration"],
            "wrongnote_flag": 1,
            "recForm": it.get("recForm", 1),
            "comment": "",
            "comment_ai": "",
            "pic_url": "",
            "ocrTids": "",
        }
        # 题级
        qa = dict(base)
        qa.update({
            "user_score": it["score"],
            "result": it["result"],
            "record_result": it.get("record_result", ""),
            "accuracy": it.get("accuracy", 0),
            "completion": it.get("completion", 0),
            "fluency": it.get("fluency", 0),
            "code": it.get("code", 0),
        })
        out.append({"elementId": el, "elementType": 1, "elementAttr": qa})
        # 句级子项：elementId = 题id-N
        for idx, s in enumerate(q["sentences"], 1):
            sa = dict(base)
            sa.update({
                "question_id": "%s-%d" % (el, idx),
                "order": idx,
                "user_score": it.get("sentence_scores", [0] * len(q["sentences"]))[idx - 1]
                if it.get("sentence_scores") else 0,
                "result": it.get("sentence_results", [1] * len(q["sentences"]))[idx - 1]
                if it.get("sentence_results") else 1,
                "record_result": it.get("sentence_results_html", [""] * len(q["sentences"]))[idx - 1]
                if it.get("sentence_results_html") else "",
                "accuracy": it.get("sentence_accuracy", [0] * len(q["sentences"]))[idx - 1]
                if it.get("sentence_accuracy") else 0,
                "completion": it.get("sentence_completion", [0] * len(q["sentences"]))[idx - 1]
                if it.get("sentence_completion") else 0,
                "fluency": it.get("sentence_fluency", [0] * len(q["sentences"]))[idx - 1]
                if it.get("sentence_fluency") else 0,
            })
            out.append({"elementId": "%s-%d" % (el, idx), "elementType": 2, "elementAttr": sa})
    return out


def build_row_branch_c(base, qst_json, answerdata, markdatao, batch_id):
    """分支 C（gzip）行字段：实测字段集（recon/readaloud-complete.md §4.4）。"""
    row = dict(base)
    row.update({
        "killStudyTask": 0,
        "batchId": batch_id,
        "extendParams": {
            "batchId": batch_id,
            "xmlResult": {"answerdata": answerdata, "markdatao": markdatao},
        },
        "qstJson": qst_json,
    })
    return row


def build_row_branch_a(base, qst_json, answerdata, markdatao, uid):
    """分支 A（form）行字段：实测字段集（recon/readaloud-complete.md §4.2）。"""
    row = dict(base)
    row.update({
        "id": uuid.uuid4().hex,
        "uid": uid,
        "batchTag": uuid.uuid4().hex,
        "result": json.dumps({
            "studentid": uid,
            "testInfo": {"answerdata": answerdata, "markdatao": markdatao},
        }, ensure_ascii=False),
        "srcType": 2,
        "uploaded": 0,
        "qstJson": qst_json,
        "msg": "",
        "failed": 0,
        "killStudyTask": 0,
        "submitType": 0,
    })
    return row


def main():
    ap = argparse.ArgumentParser(description="构造逐句跟读提交报文（默认只打印，不发送）")
    ap.add_argument("--task", default="work/readaloud-prep/task.json", help="readaloud_prepare.py 的输出")
    ap.add_argument("--scores", required=True, help="每题分数，逗号分隔，如 10,10,10")
    ap.add_argument("--urls", default="", help="每题录音的 OSS URL，逗号分隔（空则用占位 URL）")
    ap.add_argument("--branch", choices=["a", "c"], default="c", help="a=/submit/v2  c=/gzip/submit")
    ap.add_argument("--submit-type", type=int, default=8, help="submitType（真实样本用 8）")
    ap.add_argument("--book-id", default="9962655BB56EE1AD4128EFE0251D16B7")
    ap.add_argument("--study-task-id", default="")
    ap.add_argument("--course-id", type=int, default=9222783)
    ap.add_argument("--chapter-id", default="")
    ap.add_argument("--paper-id", default="")
    ap.add_argument("--uid", type=int, default=0)
    ap.add_argument("--client-id", default="")
    ap.add_argument("--seconds", type=int, default=0)
    ap.add_argument("--out", default="")
    args = ap.parse_args()

    task = json.load(open(args.task, encoding="utf-8"))
    questions = task["questions"]
    scores = [float(x) for x in args.scores.split(",")]
    if len(scores) != len(questions):
        raise SystemExit("--scores 数量(%d)与题数(%d)不符" % (len(scores), len(questions)))
    urls = [u for u in args.urls.split(",") if u] if args.urls else []

    now_ms = int(time.time() * 1000)
    items = []
    for i, q in enumerate(questions):
        url = urls[i] if i < len(urls) else "https://PLACEHOLDER/%s.mp3" % q["elementId"]
        score = scores[i]
        full = (score >= q["score"])
        items.append({
            "url": url,
            "score": score,
            # result 语义（客户端 getResultByEngineScore）：1 全对 / 2 零分 / 3 部分对
            "result": 1 if full else (2 if score == 0 else 3),
            "timestamp": now_ms,
            "duration": q["duration"],
            "record_result": "",
            "accuracy": 0,
            "completion": 0,
            "fluency": 0,
            "code": 0,
        })

    answerdata = "<elements>" + "".join(
        build_answer_element(q, it["url"], it["score"], 0, it["duration"],
                             " ".join(s["text"] for s in q["sentences"]))
        for q, it in zip(questions, items)) + "</elements>"
    markdatao = "<elements>" + "".join(
        build_mark_element(q, it["score"], it["result"]) for q, it in zip(questions, items)) + "</elements>"

    qst_json = build_qst_json(questions, items)

    base = {
        "bookId": args.book_id,
        "studyTaskId": args.study_task_id,
        "courseId": args.course_id,
        "taskId": args.chapter_id,
        "chapterId": args.chapter_id,
        "paperId": args.paper_id,
        "bookType": 2,
        "seconds": args.seconds,
        "percent": round(sum(scores) / task["totalScore"] * 100, 2),
        "score": sum(scores),
        "taskNo": 1,
        "deviceType": "Win10",
        "clientId": args.client_id,
        "studyDate": now_ms,
        "wrongnoteFlag": "PRODUCT",
        "uid": args.uid,
    }

    batch_id = uuid.uuid4().hex
    if args.branch == "c":
        row = build_row_branch_c(base, qst_json, answerdata, markdatao, batch_id)
    else:
        row = build_row_branch_a(base, qst_json, answerdata, markdatao, args.uid)

    tasks_json = json.dumps([row], ensure_ascii=False)
    ut = build_ut(tasks_json)

    if args.branch == "c":
        body = {"tasksJson": tasks_json, "ut": ut, "submitType": args.submit_type}
        url = "https://study-api.up366.cn/client/task/score/gzip/submit"
        content_type = "application/json;charset=UTF-8"
    else:
        body = {"tasksJson": tasks_json, "submitType": args.submit_type, "ut": ut}
        url = "https://study-api.up366.cn/client/task/score/submit/v2"
        content_type = "application/x-www-form-urlencoded"

    print("=== 目标接口 ===")
    print("%s  (%s)" % (url, content_type))
    print("")
    print("=== 行字段 ===")
    for k in sorted(row.keys()):
        v = row[k]
        sv = json.dumps(v, ensure_ascii=False) if isinstance(v, (dict, list)) else str(v)
        print("  %-14s %s" % (k, sv if len(sv) <= 120 else sv[:120] + "...(%d)" % len(sv)))
    print("")
    print("=== answerdata (%d 字符) ===" % len(answerdata))
    print(answerdata[:1200])
    print("")
    print("=== markdatao (%d 字符) ===" % len(markdatao))
    print(markdatao[:1200])
    print("")
    print("=== qstJson 条目 %d 条 ===" % len(qst_json))
    for e in qst_json[:4]:
        print("  " + json.dumps(e, ensure_ascii=False)[:300])
    if len(qst_json) > 4:
        print("  ... 其余 %d 条省略" % (len(qst_json) - 4))
    print("")
    print("=== ut (%d) ===" % len(ut))
    print(ut)

    if args.out:
        os.makedirs(os.path.dirname(os.path.abspath(args.out)), exist_ok=True)
        with open(args.out, "w", encoding="utf-8") as f:
            json.dump({"url": url, "contentType": content_type, "body": body,
                       "row": row, "answerdata": answerdata, "markdatao": markdatao,
                       "qstJson": qst_json, "ut": ut},
                      f, ensure_ascii=False, indent=2)
        print("")
        print("written %s" % args.out)


if __name__ == "__main__":
    main()
