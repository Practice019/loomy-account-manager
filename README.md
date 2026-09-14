# Loomy 账号管理器

独立项目：利用 token 导入并**切换本机 Loomy 桌面端登录账号**，切换前自动备份可恢复。

> 姊妹项目：[loomy-register-machine](../loomy-register-machine)（账号注册机）——注册完的账号可用本工具的 `scan` 一键纳入托管。

## 原理

本地 Loomy 登录态 = 两个文件（协议报告 §4.5）：

- `userData/auth-session.json`（electron-store：`{session, userid, phone, updatedAt}`）
- `opencode/opencode.json` 的 `provider.imodel.options.apiKey`（`useSessionAuth=true`，**apiKey 就是 session**）

切换 = 备份当前两处 → 写入新账号 session → 更新 imodel apiKey → （可选）重启 Loomy 应用生效。

## 快速开始

```bash
node manager.mjs status                 # 本机 Loomy 当前账号 + 安装根
node manager.mjs scan                   # 扫描注册机账号纳入托管（自动找兄弟目录 loomy-register-machine）
node manager.mjs list                   # 托管账号列表（标注当前激活）
node manager.mjs switch 1 --restart     # 切到第 1 个账号并重启 Loomy（无感生效）
node manager.mjs restore                # 恢复上次切换前（撤销）
```

## 给别人 token / 导入别人的 token

```bash
# 导出（把 token 文件发给对方）
node manager.mjs export <userid|手机号|序号>
# 对方收到后导入并切换
node manager.mjs import 对方发来的.json --switch
# 或者直接粘贴字段
node manager.mjs import --session <s> --userid <u> [--phone <p>] [--nickname <n>]
```

token 文件格式：`{session, userid, phone, nickname, name, updatedAt}`（export 产出，import 直接吃）。

## CLI

```
node manager.mjs status                       本机 Loomy 当前账号 + 安装根
node manager.mjs list                         托管账号列表（标注当前激活）
node manager.mjs scan [扫描目录...]           把注册机账号纳入托管（默认本目录 + 兄弟 loomy-register-machine）
node manager.mjs import <token文件> [--name 别名] [--switch]
node manager.mjs import --session <s> --userid <u> [--phone <p>] [--nickname <n>] [--name 别名] [--switch]
node manager.mjs switch <userid|手机号|序号> [--restart]
node manager.mjs export <userid|手机号|序号> [--out 路径]
node manager.mjs restore [备份名]
node manager.mjs backups
```

## Web 前端

```bash
node server.mjs          # 打开 http://127.0.0.1:3091（端口用 LOOMY_WEB_PORT 覆盖）
```

页面功能：当前激活账号 / 托管账号列表 / 一键切换（可顺带重启应用）/ **整份 token 文件粘贴导入** /
导出 token / 扫描注册机账号 / 恢复备份。只监听 127.0.0.1。

## 数据与安全

| 路径 | 内容 | 入库 |
|---|---|---|
| `managed-accounts.json` | 托管账号（含 session） | 否（gitignore） |
| `backups/<时间戳>/` | 每次切换前的登录态备份 | 否（gitignore） |
| `tokens/` | export 导出的 token 文件 | 否（gitignore） |

切换/恢复/导入/导出全部为本地文件操作，不访问网络。
