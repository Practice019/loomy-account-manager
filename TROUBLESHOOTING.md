# 故障排查

用之前先看这里 —— 多数"坏了"的情况其实是 Loomy 客户端的机制决定的，
不是缺陷。

## 目录

- [切换账号后 Loomy 还是旧账号](#切换账号后-loomy-还是旧账号)
- [找不到 Loomy / 提示"没能自动找到 Loomy"](#找不到-loomy--提示没能自动找到-loomy)
- [提示"没找到 localStorage（leveldb）"](#提示没找到-localstorageleveldb)
- [余额 / 任务 / 邀请码查不出来](#余额--任务--邀请码查不出来)
- [任务栏 / 开始菜单显示旧图标](#任务栏--开始菜单显示旧图标)
- [打开就白屏 / 显示"拒绝连接"](#打开就白屏--显示拒绝连接)
- [导入账号时报"缺少 userid"](#导入账号时报缺少-userid)
- [账号数据在哪？会上传吗？](#账号数据在哪会上传吗)
- [还是没解决？](#还是没解决)

---

## 切换账号后 Loomy 还是旧账号

**最常见的原因：切换时 Loomy 没有完全退出。**

Loomy 运行中时，它内存里有 localStorage 的副本。退出时会把那份内存里的值
刷回磁盘，于是覆盖掉我们刚写入的新账号 —— 现象就是"切换看着成功了，
重启 Loomy 又变回去"。

这是 Loomy 客户端的机制决定的，绕不过去（详见 README 的
「为什么要写三处登录态」），所以应用会强制要求先关闭它。

**做法**：在应用里点「关闭它」，或切换时确认弹窗的「确定」。
如果 Loomy 关了还是不行，用「设置 → 强制结束 Loomy」再试。

---

## 找不到 Loomy / 提示"没能自动找到 Loomy"

自动探测按这个顺序找：

```text
① 用户手动指定的路径（设置里配的）
② C:\Users\Public\Loomy\<安装ID>\userData\auth-session.json
③ %APPDATA%\Loomy\auth-session.json
```

扫不到通常是两种原因：

1. **Loomy 装在非默认位置**（绿色版、改过安装目录）
2. **Loomy 从没运行过** —— 登录态文件还没生成

**做法**：打开「设置 → 手动指定 Loomy 位置」，
填包含 `userData` 的那一层目录（通常是 `C:\Users\Public\Loomy\<安装ID>`）。
也可以直接选 `userData` 目录，或那个 `auth-session.json` 文件 —— 三种都能识别。
保存前会当场校验，路径不对会明确告诉你缺什么。

---

## 提示"没找到 localStorage（leveldb）"

说明找到了 `auth-session.json`，但 `%APPDATA%\Loomy\Local Storage\leveldb`
不存在或为空。

那是渲染进程的 localStorage —— **切换账号必须写它**，否则不生效。

**做法**：启动一次 Loomy，随便操作几下再关掉，让它生成 leveldb，
然后回应用里点「重新探测」。

---

## 余额 / 任务 / 邀请码查不出来

这些是实时调用 Loomy 官方接口，需要网络。检查：

- 网络是否通（是否走了代理，代理是否会拦 `loomyad.xunfei.cn`）
- 该账号的 session 是否还有效（在 Loomy 里还能正常用吗）
- 上游接口偶尔会慢或抖动，隔一会儿重试

报错信息里的「Loomy 接口错误 xxx」是**上游原样返回的**，不是本应用编的 ——
它通常能说明问题（例如"邀请码不存在"、"不能绑定自己账号生成的邀请码"）。

---

## 任务栏 / 开始菜单显示旧图标

Windows 会缓存程序图标。

**做法**：先**取消固定**任务栏上的图标，再重新固定。
如果还不行，删掉 `%LOCALAPPDATA%\IconCache.db` 后重启资源管理器。

---

## 打开就白屏 / 显示"拒绝连接"

如果你在**从源码运行**：

不要直接跑 `cargo run` 或 `target/debug/*.exe` —— debug 构建会去连
Vite 开发服务器（`localhost:1420`），没启动它就会显示"拒绝连接"。

用正确的命令：

```bash
bun run app:dev      # 开发模式（会自动起 Vite）
bun run app:build    # 出独立安装包
```

如果你是**装完的正式版**出现白屏，那是缺陷，请提 issue 并附上系统与版本。

---

## 导入账号时报"缺少 userid"

`userid` **无法从 session 反推** —— 必须一并提供。

**做法**：从 Loomy 的登录态文件里复制**完整内容**再粘贴：

```text
C:\Users\Public\Loomy\<安装ID>\userData\auth-session.json
```

也兼容 `uid` / `userId` 字段名。

---

## 账号数据在哪？会上传吗？

| 内容 | 位置 |
|---|---|
| 账号库 | `%APPDATA%\LoomyAccountManager\accounts.json` |
| 备份 | `%APPDATA%\LoomyAccountManager\backups\` |
| 设置 | `%APPDATA%\LoomyAccountManager\config.json` |

**不上传任何数据。** 除查询余额 / 任务 / 邀请码时调用 Loomy 官方接口外，
全部是本地文件操作。

⚠️ 那个目录里含所有账号的 session，**不要整个分享给别人**。

---

## 还是没解决？

[提一个 Bug 报告](https://github.com/Practice019/loomy-account-manager/issues/new?template=bug_report.yml)，
附上版本号、系统、复现步骤和界面上显示的报错文本。

**注意**：贴内容前请把真实的手机号 / userid / session / 邀请码替换掉。
