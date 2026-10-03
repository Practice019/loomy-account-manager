# 贡献指南

感谢你有兴趣改进这个项目。本文说明如何搭建环境、项目的约定、以及提交改动的流程。

## 环境要求

| 工具 | 版本 | 用途 |
|---|---|---|
| [Rust](https://rustup.rs/) | 1.77+ | 后端 |
| [Bun](https://bun.sh/) | 1.1+ | 前端依赖与脚本（用 npm / pnpm 也可以） |
| WebView2 | 任意 | Windows 上 Tauri 需要（Win11 内置） |

Linux 还需要 `libwebkit2gtk-4.1-dev`、`libappindicator3-dev`、
`librsvg2-dev`、`patchelf`。macOS 需要 Xcode Command Line Tools。

## 开始

```bash
git clone git@github.com:Practice019/loomy-account-manager.git
cd loomy-account-manager
bun install

bun run app:dev     # 开发模式（热重载）
```

> ⚠️ 不要直接跑 `cargo run` 或 `target/debug/*.exe` —— debug 构建会去连
> Vite 开发服务器（`localhost:1420`），没启动它就会显示"拒绝连接"。
> 要用 `bun run app:dev`（它会先起 Vite），或 `bun run app:build` 出独立程序。

## 项目结构

```
src-tauri/          Rust 后端
├─ src/
│  ├─ lib.rs            应用装配（命令注册、窗口自适应）
│  ├─ main.rs           启动壳
│  ├─ core/             路径、账号模型、配置、错误、工具 —— 不认识 Tauri
│  ├─ session/          登录态三处写入 + LevelDB 实现
│  ├─ clients/          上游 HTTP（积分网关）
│  └─ commands/         Tauri IPC 入口
├─ tests/               集成测试（真实环境，只读）
└─ examples/            开发用小工具

src/                React 前端
├─ App.tsx
├─ api/tauri.ts         invoke 的类型化封装
├─ components/
└─ lib/theme.ts         主题注册表
```

**分层原则**：`core` 与 `session` **不 import `tauri::*`**。
这样它们能在单元测试里直接跑，不必启动整个应用。

## 提交前必须通过

```bash
# 前端
bun x tsc --noEmit
bun run build

# 后端
cd src-tauri
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

CI 会在每个 PR 上跑这些检查，失败会阻止合并。

## 测试

```bash
cd src-tauri
cargo test                                  # 全部（单元 + 集成，只读，安全）
cargo test --test integration -- --nocapture  # 看真实环境观察到什么
cargo test --test e2e_switch -- --ignored     # 真的切一次账号（需先关闭 Loomy）
```

### 测试的三条约定

1. **不要碰用户的真实数据。** 需要配置/账号库的测试必须用临时目录。
   本项目踩过两次：先是直接写真实配置再"还原"，再是改用环境变量做出口
   （但 cargo 并行跑测试，环境变量是进程全局的，互相干扰）。
   正确做法是**把路径参数化**，完全不依赖全局状态。
2. **真实环境相关的测试要能跳过而不是失败。** 本机没装 Loomy 时
   `finds_real_loomy_installation` 等应当 `eprintln!` 后 `return`。
3. **不要写死任何机器相关的路径。** 曾经有测试指向开发机的网关目录，
   结果在别人机器上静默跳过 —— 那条测试对所有人都是"通过但什么也没验"。

## 修改登录态相关代码时

登录态分布在**三个**位置，写错一处就会"切换不生效"。
动手前请先读 `src-tauri/src/session/mod.rs` 与 `session/leveldb.rs` 的模块注释 ——
那里记录了实际抓到的时序证据（Loomy 渲染进程会用 localStorage 覆盖
`auth-session.json`，方向与直觉相反）。

**改这部分务必做变异验证**：注入缺陷 → 确认测试变红 → 恢复。
本项目所有关键修复都是这样验证的，不做"看起来对就行"的判断。

## 提交信息

用 [约定式提交](https://www.conventionalcommits.org/zh-hans/)：

```
<type>: <简短描述>

<可选正文，说明"为什么"而不是"是什么">
```

| type | 用途 |
|---|---|
| `feat` | 新功能 |
| `fix` | 修 bug |
| `refactor` | 重构（既不修 bug 也不加功能） |
| `test` | 增改测试 |
| `docs` | 只改文档 |
| `chore` | 工具链、依赖、配置 |

**一个提交只做一件事。** 不要把格式化改动和行为改动混在一起，
也不要一个巨型提交塞进所有东西。

## 发布流程

1. 更新 `CHANGELOG.md`（面向使用者写，按 新增/变更/修复/移除/安全 分组）
2. 同步版本号：`package.json`、`src-tauri/Cargo.toml`、`src-tauri/tauri.conf.json`
3. 提交：`chore: 发布 vX.Y.Z`
4. 打标签：`git tag -a vX.Y.Z -m "Release X.Y.Z" && git push origin vX.Y.Z`
5. Release 工作流会自动构建各平台安装包并创建 Release

## 不要提交

- 真实账号数据（手机号 / userid / session / 邀请码）—— `.gitignore` 已覆盖
  `data/`、`backups/`、`tokens/`、`managed-accounts.json`
- 构建产物（`dist/`、`src-tauri/target/`）
- 任何密钥

提交前可自查：

```bash
git diff --staged | grep -iE "session|token|secret|1[3-9][0-9]{9}"
```

## 许可

贡献的代码将以 [MIT](LICENSE) 许可发布。
