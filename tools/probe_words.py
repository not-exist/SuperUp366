import json, hashlib, os, time, urllib.request, urllib.parse, uuid, http.cookiejar

JAR = http.cookiejar.CookieJar()
OPENER = urllib.request.build_opener(urllib.request.HTTPCookieProcessor(JAR))
CLIENT_ID = '7057119429F1E0F7138F82EFF5BFE50D270DD095'
COURSE = '9222783'
TASK = 'C6E1B97AC835D37341C9F4F49FDC1719'
BUCKET = 'A56932978467152443ADE95A96C70615'

def md5(s): return hashlib.md5(s.encode('utf-8')).hexdigest()

def login(user, pw):
    p = md5(md5(pw) + 'fa&s*l%$k!fq$k!ld@fjlk')
    ts = md5(str(int(time.time()*1000)))
    ut = ts[:10] + md5(user + p + 'loginEncryptionParameter' + ts) + ts[10:]
    body = urllib.parse.urlencode({'username': user, 'password': p, 'ut': ut}).encode()
    req = urllib.request.Request('https://sso.up366.cn/v2/tickets-v3', data=body, method='POST')
    req.add_header('content-type', 'application/x-www-form-urlencoded')
    req.add_header('user-agent', 'PC-Up366-Student 6.11.0')
    req.add_header('X-App-Name', 'student-pc')
    req.add_header('clientId', CLIENT_ID)
    req.add_header('u3r', str(uuid.uuid4()))
    with OPENER.open(req, timeout=20) as r:
        j = json.loads(r.read().decode('utf-8','replace'))
    return (j.get('data') or {}).get('tgt')

def headers(tgt):
    return {'user-agent': 'PC-Up366-Student 6.11.0', 'x-requested-with': 'PC',
            'X-App-Name': 'student-pc', 'clientId': CLIENT_ID,
            'u3r': str(uuid.uuid4()), 'u3t': tgt, 'Authorization': ''}

def st_for(service, h):
    body = urllib.parse.urlencode({'service': service}).encode()
    req = urllib.request.Request('https://sso.up366.cn/v1/serviceTicket', data=body, method='POST')
    for k, v in h.items(): req.add_header(k, v)
    req.add_header('content-type', 'application/x-www-form-urlencoded')
    with OPENER.open(req, timeout=20) as r:
        return json.loads(r.read().decode('utf-8','replace'))

def call(url, method, params, tgt):
    h = headers(tgt)
    if method == 'GET':
        target = url + ('?' + urllib.parse.urlencode(params) if params else '')
        req = urllib.request.Request(target, method='GET')
    else:
        target = url
        req = urllib.request.Request(url, data=urllib.parse.urlencode(params).encode(), method='POST')
        req.add_header('content-type', 'application/x-www-form-urlencoded')
    for k, v in h.items(): req.add_header(k, v)
    with OPENER.open(req, timeout=25) as r:
        j = json.loads(r.read().decode('utf-8','replace'))
    code = (j.get('result') or {}).get('code')
    if code in (-21, -22):
        st = st_for(url, h)
        t = (st.get('data') or {}).get('serverTicket')
        if (st.get('result') or {}).get('code') == -24 and t:
            sep = '&' if '?' in target else '?'
            url2 = target + sep + 'ticket=' + urllib.parse.quote(t)
            req2 = urllib.request.Request(url2, method='GET')
            for k, v in h.items(): req2.add_header(k, v)
            with OPENER.open(req2, timeout=25) as r:
                j = json.loads(r.read().decode('utf-8','replace'))
    return j

tgt = login(os.environ['UP366_USER'], os.environ['UP366_PASS'])
print('tgt len', len(tgt or ''))
tests = [
  ('words/info', 'GET', 'https://wordsbtl-api.up366.cn/client/wordsbtl/student/words/info', {'courseId': COURSE, 'studyTaskId': TASK}),
  ('bucket', 'POST', 'https://words-v2-api.up366.cn/client/sync/teaching/bucket/detail-info', {'source': 1, 'bucketId': BUCKET}),
]
for name, m, url, params in tests:
    try:
        j = call(url, m, params, tgt)
        s = json.dumps(j, ensure_ascii=False)
        print('##', name, 'len', len(s))
        print(s[:900])
    except Exception as e:
        print('##', name, 'ERR', e)
