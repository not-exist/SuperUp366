# -*- coding: utf-8 -*-
"""逐句跟读（Unit 4 / 课文1）题目清单导出。

从本地已下载的解题包读出每题的结构，输出结构化 JSON，供后续拼音频 / 填 qstJson 用。

数据来源（全部本地实读，零联网）：
  paper.xml.u3enc        → elementId / question_type / question_score / duration
  <elId>/net/psdata/answer.json → 每题的句子原文与分词
  <elId>/net/NNNN.net            → 每句的参考音（VOASp 容器，仅记录路径）

关键事实（见 recon/readaloud-complete.md §4.9）：
  录音单位是「题」不是「句」——一次录音覆盖整题，引擎按 .net 参考音在**一次**会话里切句。
  所以 Q1(4句)/Q2(5句)/Q3(6句) 共 15 句 ⇒ 需要 3 段音频。

用法：
  python tools/readaloud_prepare.py                       # 导出到 work/readaloud-prep/task.json
  python tools/readaloud_prepare.py --out other.json
"""
import argparse
import json
import os
import re
import subprocess
import sys
import tempfile
import urllib.parse

BASE = r"C:\Up366StudentFiles\resources\2115269800157515843"
U3ENC = r"H:/project/up366/tools/u3enc.js"
OUT_DIR = r"H:/project/up366/work/readaloud-prep"

# 朗读题：question_type=9（SPOKEN_FOLLOW_READ）
SPOKEN_READ = "9"


def decrypt_paper():
    """paper.xml 已解密则直接用，否则调用 tools/u3enc.js 解密。"""
    plain = os.path.join(BASE, "paper.xml")
    if os.path.exists(plain):
        return plain
    enc = os.path.join(BASE, "paper.xml.u3enc")
    if not os.path.exists(enc):
        sys.exit("找不到 paper.xml / paper.xml.u3enc：%s" % BASE)
    tmp = os.path.join(tempfile.gettempdir(), "u4paper.xml")
    subprocess.run(["node", U3ENC, "decrypt", enc, tmp], check=True, capture_output=True)
    return tmp


def parse_paper(path):
    """从 paper.xml 抽 elementId 列表与每题元信息。"""
    s = open(path, encoding="utf-8").read()
    out = []
    for m in re.finditer(r'<element id="([^"]+)" type="([^"]+)">(.*?)</element>', s, re.S):
        eid, etype, body = m.groups()

        def tag(t):
            mm = re.search(r"<%s>(.*?)</%s>" % (t, t), body, re.S)
            return mm.group(1).strip() if mm else ""

        qtype = tag("question_type")
        if qtype != SPOKEN_READ:
            continue
        # attachment 是 URL 编码的 U3CompoLib 数据，里面是 <paragraph><sentences id="N">…<netfiles>
        att = urllib.parse.unquote_plus(tag("attachment"))
        sentences = []
        for sm in re.finditer(r"<sentences id=\"(\d+)\">(.*?)</sentences>", att, re.S):
            sid, sbody = sm.groups()
            content = re.search(r"<content><!\[CDATA\[(.*?)\]\]></content>", sbody, re.S)
            content_cn = re.search(r"<content_cn><!\[CDATA\[(.*?)\]\]></content_cn>", sbody, re.S)
            net = re.search(r'netfile="([^"]+)"', sbody)
            sentences.append({
                "id": int(sid),
                "text": (content.group(1).strip() if content else ""),
                "text_cn": (content_cn.group(1).strip() if content_cn else ""),
                "netfile": (net.group(1) if net else ""),
            })
        out.append({
            "elementId": eid,
            "question_no": int(tag("question_no") or 0),
            "question_type": int(qtype),
            "score": float(tag("question_score") or 0),
            "duration": int(tag("duration") or 0),
            "ref_question_id": tag("ref_question_id"),
            "paragraphCount": len(sentences),
            "sentences": sentences,
        })
    return out


def parse_answer_json(el_id):
    """answer.json 的 Data.sentences[] 与 paper.xml 的 sentences 对齐校验用。"""
    p = os.path.join(BASE, el_id, "net", "psdata", "answer.json")
    if not os.path.exists(p):
        return None
    d = json.load(open(p, encoding="utf-8"))
    data = d.get("Data") or {}
    return {
        "Type": d.get("Type"),
        "SubType": d.get("SubType"),
        "QuestionType": d.get("QuestionType"),
        "QuestionTypeV3": d.get("QuestionTypeV3"),
        "ResponseMode": d.get("ResponseMode"),
        "sentences": [
            {
                "text": s.get("text", ""),
                "words": s.get("words", []),
                "sense_group": s.get("sense_group", []),
                "punctuation_index": s.get("punctuation_index", []),
            }
            for s in (data.get("sentences") or [])
        ],
    }


def main():
    ap = argparse.ArgumentParser(description="导出逐句跟读题目清单")
    ap.add_argument("--out", default=os.path.join(OUT_DIR, "task.json"))
    args = ap.parse_args()

    paper = decrypt_paper()
    questions = parse_paper(paper)

    total_sentences = 0
    for idx, q in enumerate(questions, 1):
        aj = parse_answer_json(q["elementId"])
        q["answer_json"] = aj
        q["net_dir"] = os.path.join(BASE, q["elementId"], "net")
        # paper.xml 里每题自己的 question_no 都是 1，改用整体序号，便于人读与映射
        q["index"] = idx
        q["media_mp3"] = None
        media_dir = os.path.join(BASE, q["elementId"], "media")
        if os.path.isdir(media_dir):
            mp3s = sorted(f for f in os.listdir(media_dir) if f.lower().endswith(".mp3"))
            if mp3s:
                q["media_mp3"] = os.path.join(media_dir, mp3s[0])
        # 一致性校验：paper.xml 的句子数 vs answer.json 的句子数
        if aj is not None and len(aj["sentences"]) != q["paragraphCount"]:
            print("[warn] %s paper.xml 句数 %d != answer.json 句数 %d"
                  % (q["elementId"], q["paragraphCount"], len(aj["sentences"])), file=sys.stderr)
        total_sentences += q["paragraphCount"]

    result = {
        "source": BASE,
        "paperXml": paper,
        "recordingUnit": "question",
        "questionCount": len(questions),
        "sentenceCount": total_sentences,
        "totalScore": sum(q["score"] for q in questions),
        "questions": questions,
    }
    os.makedirs(os.path.dirname(args.out), exist_ok=True)
    with open(args.out, "w", encoding="utf-8") as f:
        json.dump(result, f, ensure_ascii=False, indent=2)

    print("题数=%d  句数=%d  总分=%g" % (result["questionCount"], total_sentences, result["totalScore"]))
    for q in questions:
        print("  Q%d %s  句数=%d  分值=%g  时长=%ds  参考音=%s"
              % (q["index"], q["elementId"], q["paragraphCount"], q["score"], q["duration"],
                 os.path.basename(q["media_mp3"] or "-")))
        for s in q["sentences"]:
            print("     #%d %s  %s" % (s["id"], s["netfile"], s["text"][:70]))
    print("written %s" % args.out)


if __name__ == "__main__":
    main()
