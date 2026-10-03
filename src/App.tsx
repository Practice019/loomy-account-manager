/**
 * 应用骨架 —— 抄 Kiro 的 App 结构（hj01857655/kiro-account-manager）。
 *
 * # 抄了什么
 *
 * ```text
 * <div class="flex h-screen">
 *   <Sidebar />                                  ← 左侧导航（可折叠）
 *   <main>{renderContent()}</main>               ← 主内容区，按 activeMenu 切换
 *   <Toaster position="top-center" />            ← 全局提示
 * </div>
 * ```
 *
 * 三处照抄的设计决策：
 *
 * 1. **`activeMenu` 存 localStorage** —— 重开应用回到上次那页。
 * 2. **窗口先 `visible:false` 再 show** —— 避免白屏一闪（它在
 *    `requestAnimationFrame` 里 show，等价效果）。
 * 3. **提示条在顶部居中**，而不是右下角 —— 桌面应用里居中更醒目。
 */

import { useCallback, useEffect, useState } from "react"
import { getVersion } from "@tauri-apps/api/app"
import { AlertTriangle, CheckCircle2, Info } from "lucide-react"
import Sidebar from "./components/Sidebar"
import type { PageId } from "./components/Sidebar"
import Dashboard from "./components/Dashboard"
import AccountsPage from "./components/AccountsPage"
import ImportPanel from "./components/ImportPanel"
import SettingsPage from "./components/SettingsPage"
import { api, asBackendError, errorText } from "./api/tauri"
import type { AccountView, BackupMeta, SwitchPrecheck } from "./api/tauri"
import { applyTheme } from "./lib/theme"

export type Toast = { kind: "ok" | "err" | "info" | "warn"; text: string }

const PAGE_TITLES: Record<PageId, string> = {
  dashboard: "仪表盘",
  accounts: "账号管理",
  import: "导入账号",
  settings: "设置",
}

export default function App() {
  const [accounts, setAccounts] = useState<AccountView[]>([])
  const [activeUserid, setActiveUserid] = useState<string | null>(null)
  const [precheck, setPrecheck] = useState<SwitchPrecheck | null>(null)
  const [backups, setBackups] = useState<BackupMeta[]>([])
  const [toast, setToast] = useState<Toast | null>(null)
  const [busy, setBusy] = useState(false)
  const [loading, setLoading] = useState(true)

  // 抄 Kiro：当前页存 localStorage，重开回到同一页
  const [page, setPage] = useState<PageId>(() => {
    const saved = localStorage.getItem("activeMenu")
    const valid: PageId[] = ["dashboard", "accounts", "import", "settings"]
    return valid.includes(saved as PageId) ? (saved as PageId) : "dashboard"
  })

  // 侧栏折叠状态也持久化（抄 Kiro 的 sidebar-collapsed）
  const [collapsed, setCollapsed] = useState(
    () => localStorage.getItem("sidebar-collapsed") === "true",
  )

  const [theme, setTheme] = useState(() => localStorage.getItem("theme") || "dark")

  /**
   * 应用版本号 —— 从 Tauri 读，**不硬编码**。
   *
   * 硬编码的后果：改版本时得记着同步这一处，漏了就显示旧版本号
   * （而界面上显示错误版本会让人误判"我装的是不是新版"）。
   * Tauri 的 `getVersion()` 直接读 `tauri.conf.json`，是唯一事实来源。
   *
   * 读不到时回落到占位符而不是某个具体版本 —— 那比显示一个错的好。
   */
  const [appVersion, setAppVersion] = useState("…")
  useEffect(() => {
    void getVersion()
      .then(setAppVersion)
      .catch(() => setAppVersion("—"))
  }, [])

  useEffect(() => {
    localStorage.setItem("activeMenu", page)
  }, [page])

  useEffect(() => {
    localStorage.setItem("sidebar-collapsed", String(collapsed))
  }, [collapsed])

  // 主题：写 <html data-theme>，并监听系统偏好（system 主题下要跟着变）
  useEffect(() => {
    localStorage.setItem("theme", theme)
    applyTheme(theme)
    if (theme !== "system") return
    const mq = window.matchMedia("(prefers-color-scheme: dark)")
    const onChange = () => applyTheme("system")
    mq.addEventListener("change", onChange)
    return () => mq.removeEventListener("change", onChange)
  }, [theme])

  const notify = useCallback((kind: Toast["kind"], text: string) => {
    setToast({ kind, text })
  }, [])

  const refresh = useCallback(async () => {
    try {
      const [list, pc, bk, active] = await Promise.all([
        api.listAccounts(),
        api.precheck(),
        api.listBackups(),
        api.activeAccount(),
      ])
      setAccounts(list)
      setPrecheck(pc)
      setBackups(bk)
      setActiveUserid(active?.userid ?? pc.activeUserid ?? null)
    } catch (e) {
      notify("err", "读取状态失败：" + errorText(e))
    } finally {
      setLoading(false)
    }
  }, [notify])

  useEffect(() => {
    void refresh()
  }, [refresh])

  // 提示条自动消失（错误停留更久，因为要读）
  useEffect(() => {
    if (!toast) return
    const t = setTimeout(() => setToast(null), toast.kind === "err" ? 14000 : 6000)
    return () => clearTimeout(t)
  }, [toast])

  const doSwitch = useCallback(
    async (userid: string, restartAfter: boolean) => {
      const target = accounts.find((a) => a.userid === userid)
      if (!target) return

      let killRunning = false
      if (precheck && precheck.runningPids.length > 0) {
        const ok = confirm(
          `Loomy 正在运行（${precheck.runningPids.length} 个进程）。\n\n` +
            `切换账号必须先完全退出它 —— 否则 Loomy 退出时会把内存里的旧账号写回磁盘，` +
            `本次切换会被覆盖。\n\n` +
            `要我帮你关掉它吗？（点"取消"则中止本次切换）`,
        )
        if (!ok) return
        killRunning = true
      }

      setBusy(true)
      try {
        const r = await api.switchAccount(userid, killRunning, restartAfter)
        const parts = [`已切到 ${target.displayName}`]
        if (r.applied.leveldbLogs.length > 0) {
          parts.push(`localStorage 已更新（关键，${r.applied.leveldbLogs.length} 处）`)
        } else {
          parts.push("⚠ localStorage 未写入 —— 切换可能不生效")
        }
        parts.push(r.restarted ? "Loomy 已重启" : "请手动启动 Loomy 使新账号生效")
        parts.push(`已备份原账号（${r.backup}）`)
        notify(r.applied.leveldbLogs.length > 0 ? "ok" : "warn", parts.join("\n"))
        if (r.applied.warnings.length) notify("warn", r.applied.warnings.join("\n"))
        await refresh()
      } catch (e) {
        const be = asBackendError(e)
        if (be.kind === "loomyRunning") {
          notify("err", be.message + "\n\n请手动完全退出 Loomy 后重试。")
        } else {
          notify("err", "切换失败：" + be.message)
        }
      } finally {
        setBusy(false)
      }
    },
    [accounts, precheck, notify, refresh],
  )

  const running = (precheck?.runningPids.length ?? 0) > 0
  const connection = precheck?.root
    ? running
      ? { ok: false, text: `Loomy 运行中（${precheck.runningPids.length}）` }
      : { ok: true, text: "Loomy 未运行" }
    : { ok: false, text: "未找到 Loomy" }

  return (
    <div className="app">
      <Sidebar
        active={page}
        onNavigate={setPage}
        collapsed={collapsed}
        onToggleCollapse={() => setCollapsed((c) => !c)}
        theme={theme}
        onThemeChange={setTheme}
        version={appVersion}
        connection={connection}
      />

      <main className="main">
        {/* 顶部条（抄 Kiro 的页头） */}
        <div className="main-head">
          <h1>{PAGE_TITLES[page]}</h1>
          {page === "accounts" && (
            <span className="dim" style={{ fontSize: 12 }}>
              {accounts.length} 个账号
            </span>
          )}
          <div className="spacer" />
          {running && (
            <span className="badge off">
              <AlertTriangle size={11} /> Loomy 运行中
            </span>
          )}
          <button className="btn sm" onClick={() => void refresh()} disabled={busy}>
            {loading ? <span className="spin" /> : "刷新"}
          </button>
        </div>

        <div className="main-body">
          {/* 提示条（顶部居中，抄 Kiro 的 Toaster position） */}
          {toast && (
            <div className={`alert ${toast.kind} fade-in`}>
              {toast.kind === "ok" ? (
                <CheckCircle2 size={15} className="ico" />
              ) : toast.kind === "err" || toast.kind === "warn" ? (
                <AlertTriangle size={15} className="ico" />
              ) : (
                <Info size={15} className="ico" />
              )}
              <div className="body">{toast.text}</div>
              <button className="btn sm ghost" onClick={() => setToast(null)}>
                ✕
              </button>
            </div>
          )}

          {/* 切换前的运行中警告（只在账号相关页显示） */}
          {running && (page === "accounts" || page === "dashboard") && (
            <div className="alert warn">
              <AlertTriangle size={15} className="ico" />
              <div className="body">
                Loomy 正在运行。切换账号前必须先完全退出它 —— 否则它退出时会把内存里的旧账号
                写回磁盘，本次切换会被覆盖（Loomy 客户端机制决定的，绕不过去）。
                切换时会提示你确认。
              </div>
            </div>
          )}

          {page === "dashboard" && (
            <Dashboard
              accounts={accounts}
              activeUserid={activeUserid}
              onNavigate={(p) => setPage(p)}
              busy={busy}
              notify={notify}
            />
          )}

          {page === "accounts" && (
            <AccountsPage
              accounts={accounts}
              activeUserid={activeUserid}
              busy={busy}
              onSwitch={doSwitch}
              onRefresh={refresh}
              notify={notify}
            />
          )}

          {page === "import" && (
            <ImportPanel
              notify={notify}
              onDone={async () => {
                await refresh()
                setPage("accounts")
              }}
            />
          )}

          {page === "settings" && (
            <SettingsPage
              theme={theme}
              onThemeChange={setTheme}
              backups={backups}
              onRefresh={refresh}
              notify={notify}
            />
          )}
        </div>
      </main>
    </div>
  )
}
