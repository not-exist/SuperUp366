# 动态码登录（短信验证码登录）API — 调用架构

> 背景：平板版更新后，部分账号登录时要求输入「动态码」才能成功。
> 结论：**动态码登录 = 客户端里一直存在的「手机验证码登录」通道**，端到端链路在 6.11.0 PC 客户端里就有完整实现；
> 平板版只是把部分账号强制引导到这条通道（触发在服务端），并没看到新的 sso 端点。
> 证据范围：PC 学生端 6.11.0（extract/）、教师端 6.14.0 增量包（work/teacher-6.14.0-dec/，2025-12-26 构建）。
> 平板版二进制未获取 —— 见文末「未决」。

---

## 1. 两条腿的接口

### ① 发送动态码

```
GET https://user-api.up366.cn/front/user/verify/send
    ?checkExists=2
    &mobile=<手机号>
    &ut=<ut>
```

```js
// main/index.js@3562916（CF.sendVerifyCode / wR.sendVerifyCode）
sendVerifyCode(e, t) {                     // e=手机号, t=checkExists（PC 传 "2"）
  const i = md5(Date.now().toString())     // ts
  const ut = i.slice(0, 10)
            + md5(`${e}sendSMSToken${i}`)  // 关键词 sendSMSToken，payload=手机号
            + i.slice(10)
  return this.service.getByName("sendVerify", { checkExists: t, mobile: e, ut })
}
```

### ② 动态码登录

```
POST https://sso.up366.cn/v2/code/tickets
Content-Type: application/json (由桥转发，字段原样透传)

{"mobile": "...", "verifyCode": "..."}                        // 学生端 6.11.0 只发这两个
{"userName": "...", "password": "...", "mobile": "...",       // 教师端 6.14.0 额外透传
 "verifyCode": "...", "loginPath": "https://laoshi.up366.cn"}
```

```js
// main/index.js@3562970
verifyCodeLogin(e) { return this.service.http.post(this.service.config.verifyCodeLogin, e).json() }
// config.verifyCodeLogin = https://sso.up366.cn/v2/code/tickets  @3561693
```

成功判定：`result.code === 0 || -26`（Web 版两个都当成功），`data.tgt` 即 `u3t`。

## 2. UI 调用链（PC 学生端）

```
登录页 手机 tab（res/static/login/login.bdb7e7c2.js@259413 codeLogin）
  if (isClient) Login.verifyCodeLogin(JSON.stringify({mobile, verifyCode}))
    → 主进程 CF.verifyCodeLogin → POST /v2/code/tickets
  else（Web 版）→ He({mobile, verifyCode}) → 0 / -26 算成功
```

发码按钮：`sendYzm()` → `Login.sendVerifyCode(phone)` → GET verify/send。

## 3. 教师端 6.14.0（2025-12-26 构建）的最新形态

`work/teacher-6.14.0-dec/res/login/Login.html:1615`：

```js
// 短信登录分支（mobile+verifyCode 有值时）
userInfo.userName  = count       // 账号可同时带上
userInfo.password  = password    // 密码也可同时带上
userInfo.mobile    = mobile
userInfo.verifyCode = verifyCode
userInfo.loginPath = 'https://laoshi.up366' + getPlatform() + '.cn'
Login.verifyCodeLogin(JSON.stringify(userInfo))
```

即 **`/v2/code/tickets` 接受 账号+密码+手机+验证码 的任意组合** ——
「部分账号需要动态码」的实现方式大概率就是：服务端对 `tickets-v3`（密码登录）按账号返回特定错误码，
客户端（平板版）收到后弹出动态码输入，走 `verify/send` + `/v2/code/tickets` 补一次登录。

## 4. 未决 / 边界

- **平板版二进制未获取**：`front/app/version/list` 上没有平板/Android 的 modelName
  （探过 student-android-*、teacher-android-*、student-pad-* 等，均无条目；只有 win32 的 student/teacher 两族）。
  所以「平板版是否有新增端点」无法从二进制确认。
- **没有真实触发过**：需要动态码的账号不在我们手里，`verify/send` 会发真实短信，未做实测。
- **错误码表（旧版 @374154）**里没有"需要动态码"的码（-21~-31、-39），平板版大概率扩了表。
