# -*- coding: utf-8 -*-
"""逐句跟读：上传 3 段 mp3 → 写 TaskStuded 行 → 让客户端自己提交。

为什么这么设计（全部实测，见 recon/readaloud-complete.md §4）：
  * 提交走**分支 A**：`POST /client/task/score/submit/v2`
      body = {tasksJson: "<JSON.stringify([row])>", submitType, ut}
      ut = r[:10] + md5(md5(tasksJson) + "submitTaskToken-pc-987654" + r) + r[10:]   （已用真实报文验证）
  * 但 ut 由**客户端自己算**最稳：写行 → 客户端 `startUpload()` 扫表 → `uploadScore()`
    → `submitTaskScore()`。登录态、表单编码、ST 兜底、重试队列全由客户端处理。
  * 音频先传 OSS：`AppInfo.uploadFile(path, objectName)`（客户端自己的 STS）。
  * 音频与分数的关联有两处，都要填：
      - `qstJson[].elementAttr.record_url`（题级一个 URL）
      - `result.testInfo.answerdata` 的 `<answer_url>`
  * `TaskStuded.qstJson` 在库里的形态是**对象**（键 `${elementId}_${order}`），
    客户端上传前会拍平成数组（`uploadScore`）。

⚠️ 一次性：同一 (uid, studyTaskId) 服务端判重，第二次返回 -13 重复作答。

用法：
  python tools/readaloud_submit.py --mp3 q1=..,q2=..,q3=.. --scores 8,8,8          # dry-run
  python tools/readaloud_submit.py --mp3 ... --scores 8,8,8 --send                 # 真提交
"""
import argparse
import json
import os
import re
import subprocess
import sys
import time

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
USER = os.environ.get("UP366_USER", "")
UID = 39370562
USER_TYPE = 102
CLIENT_ID = "7057119429F1E0F7138F82EFF5BFE50D270DD095"

JOB = {
    "contentId": "3D1B42C891F7080656123366A77DD5A7",
    "paperId": "1D758BD86E8BE3AF0797789D68EF531B",
    "bookId": "9962655BB56EE1AD4128EFE0251D16B7",
    "courseId": 9222783,
    "taskScore": 30,
}

PKG = r"C:\Up366StudentFiles\resources\2115269800157515843"
QUESTIONS = [
    ("35022420E8C14F9B94FC490E457EDED9", 4, 10.0, 82),
    ("E58008A83034E394A4665B5FAD7138EE", 5, 10.0, 86),
    ("666A4C22F65F8DDB243A8583ABE1AA7B", 6, 10.0, 135),
]


def eval_in_page(js, timeout=180):
    env = dict(os.environ)
    env["MSYS_NO_PATH_CONV"] = "1"
    env["U3_FRAME"] = "=local://res/index.html#/"
    p = subprocess.run(["node", os.path.join(REPO, "harness", "u3.js"), "eval", js],
                       capture_output=True, text=True, env=env, timeout=timeout)
    out = (p.stdout or "").strip()
    if p.returncode != 0:
        raise RuntimeError("页面执行失败: %s %s" % (out, (p.stderr or "").strip()[:300]))
    return out


def load_questions():
    import urllib.parse
    paper = os.path.join(PKG, "paper.xml")
    if not os.path.exists(paper):
        tmp = os.path.join(REPO, "work", "task_unit4", "paper.xml")
        subprocess.run(["node", os.path.join(REPO, "tools", "u3enc.js"), "decrypt",
                        os.path.join(PKG, "paper.xml.u3enc"), tmp], check=True, capture_output=True)
        paper = tmp
    data = open(paper, encoding="utf-8").read()
    qs = []
    for el_id, n_sent, score, dur in QUESTIONS:
        m = re.search(r'<element id="%s" type="3">(.*?)</element>' % el_id, data, re.S)
        att = re.search(r"<attachment>(.*?)</attachment>", m.group(1), re.S)
        att = urllib.parse.unquote_plus(att.group(1).strip()) if att else ""
        sents = []
        for sm in re.finditer(r"<sentences id=\"(\d+)\">(.*?)</sentences>", att, re.S):
            c = re.search(r"<content><!\[CDATA\[(.*?)\]\]></content>", sm.group(2), re.S)
            sents.append(c.group(1).strip() if c else "")
        qs.append({"elementId": el_id, "sentences": sents, "score": score, "duration": dur})
    return qs


def build(questions, scores, urls, seconds):
    ts = int(time.time() * 1000)
    qst = {}
    ad, md = [], []
    for q, score, url in zip(questions, scores, urls):
        el = q["elementId"]
        n = len(q["sentences"])
        result = 1 if score >= q["score"] else (2 if score == 0 else 3)
        attr = {
            "question_id": el, "question_type": 9, "order": 1,
            "user_score": score, "user_answer": "", "answer": "",
            "score": q["score"], "result": result, "timestamp": ts,
            "record_url": url, "duration": q["duration"],
            "wrongnote_flag": 1, "recForm": 1, "comment": "", "comment_ai": "",
            "pic_url": "", "ocrTids": "",
            "record_result": "", "accuracy": 0, "completion": 0, "fluency": 0, "code": 0,
        }
        qst["%s_1" % el] = {"elementId": el, "elementType": 1, "elementAttr": attr}
        for i in range(1, n + 1):
            sa = dict(attr)
            sa.update({"question_id": "%s-%d" % (el, i), "order": i, "user_score": score, "result": result})
            qst["%s-%d_%d" % (el, i, i)] = {"elementId": "%s-%d" % (el, i), "elementType": 2,
                                            "elementAttr": sa}
        text = " ".join(q["sentences"]).replace("]]>", "]]]]><![CDATA[>")
        ad.append('<element id="%s"><answers><answer_url>%s</answer_url>'
                  "<answer_score>%s</answer_score><answer_rejectscore>0</answer_rejectscore>"
                  "<answer_text><![CDATA[%s]]></answer_text>"
                  "<answer_recordmin><![CDATA[0]]></answer_recordmin>"
                  "<answer_recordsec><![CDATA[%d]]></answer_recordsec></answers></element>"
                  % (el, url, score, text, q["duration"]))
        md.append('<element id="%s"><mark_data><score>%s</score><result>%s</result>'
                  "</mark_data></element>" % (el, score, result))

    total = sum(scores)
    batch = os.urandom(16).hex()
    row = {
        "id": os.urandom(16).hex(),
        "taskId": JOB["contentId"],
        "uid": UID,
        "bookId": JOB["bookId"],
        "chapterId": JOB["contentId"],
        "taskNo": 1,
        "score": total,
        "percent": round(total / JOB["taskScore"] * 100, 2),
        "studyDate": ts,
        "seconds": seconds,
        "result": json.dumps({"studentid": UID, "testInfo": {
            "answerdata": "<elements>" + "".join(ad) + "</elements>",
            "markdatao": "<elements>" + "".join(md) + "</elements>"}}, ensure_ascii=False),
        "srcType": 2,
        "batchTag": os.urandom(16).hex(),
        "uploaded": 0,
        "msg": "",
        "studyTaskId": os.urandom(16).hex(),
        "courseId": str(JOB["courseId"]),
        "extendParams": json.dumps({"batchId": batch}, ensure_ascii=False),
        "qstJson": json.dumps(qst, ensure_ascii=False),
        "failed": 0,
        "killStudyTask": 0,
        "submitType": 8,
    }
    return row


def main():
    ap = argparse.ArgumentParser(description="逐句跟读提交（默认 dry-run）")
    ap.add_argument("--mp3", required=True, help="q1=..,q2=..,q3=..")
    ap.add_argument("--scores", required=True, help="8,8,8")
    ap.add_argument("--send", action="store_true")
    ap.add_argument("--no-upload", action="store_true", help="复用 work/readaloud-prep/urls.json 里已上传的 URL")
    args = ap.parse_args()

    pairs = [(k.strip(), v.strip()) for k, v in (x.split("=", 1) for x in args.mp3.split(",") if x.strip())]
    if len(pairs) != 3:
        sys.exit("--mp3 需要 3 段")
    scores = [float(x) for x in args.scores.split(",")]
    if len(scores) != 3:
        sys.exit("--scores 需要 3 个")

    qs = load_questions()
    total = sum(scores)

    # 时长（秒）—— 用真实音频时长，submitted 时作为 seconds 上报
    durs = []
    for _, p in pairs:
        r = subprocess.run([r"C:/Program Files (x86)/up366/client/lib/ia32/win32/ffmpeg/ffprobe.exe",
                            "-v", "error", "-show_entries", "format=duration",
                            "-of", "default=noprint_wrappers=1:nokey=1", p],
                           capture_output=True, text=True)
        durs.append(float((r.stdout or "0").strip() or 0))
    seconds = int(sum(durs))

    print("=== 目标 ===")
    print("  Unit 4 课文1 / 逐句跟读   变体 A（本地包 2115269800157515843）")
    print("  taskId=%s  paperId=%s" % (JOB["contentId"], JOB["paperId"]))
    print("  分数 %s / %d = %.0f%%   时长 %ds"
          % ([int(s) for s in scores], JOB["taskScore"], total / JOB["taskScore"] * 100, seconds))
    for q, s, (k, p) in zip(qs, scores, pairs):
        print("  %-34s %d 句  %d/%g 分  %s（%ds）" % (q["elementId"], len(q["sentences"]), s, q["score"], k, q["duration"]))

    if not args.send:
        print("\n[dry-run] 未提交。加 --send 执行。")
        return

    # 1) 上传
    urls = []
    cache = os.path.join(REPO, "work", "readaloud-prep", "urls.json")
    if args.no_upload and os.path.exists(cache):
        urls = json.load(open(cache, encoding="utf-8"))["urls"]
        print("\n[upload] 复用已上传 URL")
    else:
        print("\n=== 上传音频 ===")
        js = """
(async () => {
  const files = %s, uid = %d, userType = %d;
  const p2 = n => (n < 10 ? '0' : '') + n;
  const d = new Date();
  const dp = d.getFullYear() + '/' + p2(d.getMonth() + 1) + '/' + p2(d.getDate());
  const out = [];
  for (let i = 0; i < files.length; i++) {
    const name = 'ra' + Date.now().toString(16) + i + '.mp3';
    const objectName = 'storage' + (userType ? '/' + userType : '') + '/study/a/' + dp +
      (uid ? '/' + uid : '') + '/' + name;
    const r = await window.AppInfo.uploadFile(files[i], objectName);
    if (!r || !r.result || r.result.code !== 0) return JSON.stringify({ ok:false, file: files[i], resp: r });
    out.push({ path: files[i], objectName: objectName, url: r.data.url });
  }
  return JSON.stringify({ ok: true, files: out });
})()
""" % (json.dumps([p for _, p in pairs]), UID, USER_TYPE)
        res = json.loads(eval_in_page(js))
        if not res.get("ok"):
            sys.exit("上传失败：" + json.dumps(res, ensure_ascii=False)[:400])
        urls = [f["url"] for f in res["files"]]
        for f in res["files"]:
            print("  %s\n    → %s" % (os.path.basename(f["path"]), f["url"]))
        os.makedirs(os.path.dirname(cache), exist_ok=True)
        json.dump({"urls": urls}, open(cache, "w", encoding="utf-8"), ensure_ascii=False, indent=2)

    # 2) 构造行
    row = build(qs, scores, urls, seconds)
    os.makedirs(os.path.join(REPO, "work", "readaloud-prep"), exist_ok=True)
    payload = os.path.join(REPO, "work", "readaloud-prep", "row.json")
    json.dump(row, open(payload, "w", encoding="utf-8"), ensure_ascii=False, indent=2)
    print("\n=== 待提交行 ===")
    print("  studyTaskId=%s" % row["studyTaskId"])
    print("  score=%g percent=%g qstJson=%d 条 result=%d 字符 seconds=%d"
          % (row["score"], row["percent"], len(json.loads(row["qstJson"])), len(row["result"]), row["seconds"]))

    # 3) 备份库 → 写行 → 触发客户端上传器
    print("\n=== 写库 ===")
    env = dict(os.environ)
    env["ELECTRON_RUN_AS_NODE"] = "1"
    exe = r"C:/Program Files (x86)/up366/client/up366.exe"
    subprocess.run([exe, "-e", open(os.path.join(REPO, "tools", "read_setupcfg.js"), encoding="utf-8").read(),
                    "backup", "--user", USER], env=env, check=False, capture_output=True)
    r = subprocess.run([exe, "-e", open(os.path.join(REPO, "tools", "read_setupcfg.js"), encoding="utf-8").read(),
                        "insert-score", "--user", USER, "--json", payload],
                       env=env, capture_output=True, text=True)
    print((r.stdout or "").strip()[:800])

    # 4) 触发客户端提交
    print("\n=== 触发客户端提交 ===")
    out = eval_in_page("window.NetBrowser.manualUploadTask()")
    print("  " + out[:200])
    print("\n等待 8s 观察结果…")
    time.sleep(8)
    print((subprocess.run([exe, "-e", open(os.path.join(REPO, "tools", "read_setupcfg.js"), encoding="utf-8").read(),
                           "dump", "--user", USER], env=env, capture_output=True, text=True).stdout or "")[:1600])


if __name__ == "__main__":
    main()
