/**
 * 设置页 —— 外观、环境（真实状态 + 手动指定）、备份、其它。
 *
 * # 环境这一节修的是一个真 bug
 *
 * 第一版这样显示"数据目录"：
 *
 * ```ts
 * listBackups().then(() => setDataDir("%APPDATA%\\LoomyAccountManager"))
 * ```
 *
 * 那个字符串是**前端写死的字面量**，跟后端毫无关系。如果实际数据目录
 * 不是这个（换了环境变量、或走了兜底分支），界面会显示一个错误的位置，
 * 用户照着去找会一无所获。
 *
 * 现在改为调 `system_status`，由后端报告**它自己实际用的**路径。
 */

import { useCallback, useEffect, useState } from "react"
import { AlertTriangle, CheckCircle2, FolderOpen, Info, Palette, RotateCcw, Search, Trash2, XCircle } from "lucide-react"
import { open } from "@tauri-apps/plugin-dialog"
import { THEME_LIST } from "../lib/theme"
import type { BackupMeta, SystemStatus } from "../api/tauri"
import { api, errorText } from "../api/tauri"
import type { Toast } from "../App"

export default function SettingsPage({
  theme,
  onThemeChange,
  backups,
  onRefresh,
  notify,
}: {
  theme: string
  onThemeChange: (t: string) => void
  backups: BackupMeta[]
  onRefresh: () => void | Promise<void>
  notify: (k: Toast["kind"], t: string) => void
}) {
  const [busy, setBusy] = useState(false)
  const [status, setStatus] = useState<SystemStatus | null>(null)
  const [manualInput, setManualInput] = useState("")
  const [saving, setSaving] = useState(false)

  const loadStatus = useCallback(async () => {
    try {
      setStatus(await api.systemStatus())
    } catch (e) {
      notify("err", "读取环境失败：" + errorText(e))
    }
  }, [notify])

  useEffect(() => {
    void loadStatus()
  }, [loadStatus])

  /** 选一个目录（优先用系统对话框，不可用时退回手动输入）。 */
  const pickDir = async () => {
    try {
      const picked = await open({
        directory: true,
        multiple: false,
        title: "选择 Loomy 的安装目录（含 userData 的那一层）",
      })
      if (typeof picked === "string") setManualInput(picked)
    } catch {
      // 没有 dialog 权限 —— 让用户手输（输入框本来就在，这里只是不动而已）
      notify("info", "无法打开文件选择器，请把路径粘贴到下方输入框")
    }
  }

  const saveManual = async (path: string) => {
    const p = path.trim()
    if (!p) {
      notify("warn", "先填一个路径")
      return
    }
    setSaving(true)
    try {
      const resolved = await api.setLoomyPath(p)
      notify("ok", `已记住 Loomy 位置：\n${resolved}`)
      setManualInput("")
      await loadStatus()
      await onRefresh()
    } catch (e) {
      // 后端的报错里已说明"要选到哪一层"，原样显示
      notify("err", errorText(e))
    } finally {
      setSaving(false)
    }
  }

  const clearManual = async () => {
    setSaving(true)
    try {
      await api.clearLoomyPath()
      notify("ok", "已清除手动指定，回到自动探测")
      await loadStatus()
      await onRefresh()
    } catch (e) {
      notify("err", errorText(e))
    } finally {
      setSaving(false)
    }
  }

  const restore = async (name: string) => {
    if (!confirm(`恢复备份 ${name}？\n\n当前 Loomy 登录态会被替换为该备份的账号。`)) return
    setBusy(true)
    try {
      const r = await api.restoreBackup(name)
      const lines = [`已恢复 → ${r.userid || "(空)"}`]
      if (r.applied.leveldbLogs.length > 0) lines.push("localStorage 已同步恢复 ✓")
      else lines.push("⚠ localStorage 未恢复 —— 可能不生效")
      if (r.applied.warnings.length) lines.push(...r.applied.warnings)
      notify("ok", lines.join("\n"))
      await onRefresh()
    } catch (e) {
      notify("err", "恢复失败：" + errorText(e))
    } finally {
      setBusy(false)
    }
  }

  const fmt = (ms: number) => {
    if (!ms) return "-"
    const d = new Date(ms)
    const p = (n: number) => String(n).padStart(2, "0")
    return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())} ${p(d.getHours())}:${p(d.getMinutes())}:${p(d.getSeconds())}`
  }

  const st = status
  const running = (st?.runningPids.length ?? 0) > 0

  return (
    <div className="fade-in">
      {/* ── 环境 ─────────────────────────────────────────────────────── */}
      <div className="card">
        <h2>
          <Info size={15} /> 环境
          <span className="spacer" />
          <button className="btn sm ghost" onClick={() => void loadStatus()}>
            重新探测
          </button>
        </h2>

        {/* 探测失败 / 有歧义时的明确提示 + 手动指定入口 */}
        {st?.needsManual && (
          <div className="alert warn">
            <AlertTriangle size={15} className="ico" />
            <div className="body">
              <strong>{st.found ? "需要你指定用哪个 Loomy" : "没能自动找到 Loomy"}</strong>
              {"\n"}
              {st.reason}
            </div>
          </div>
        )}

        {/* 找到了但缺 leveldb —— 不阻塞使用，但切换会不生效，必须警告 */}
        {st && !st.needsManual && st.reason && (
          <div className="alert warn">
            <AlertTriangle size={15} className="ico" />
            <div className="body">{st.reason}</div>
          </div>
        )}

        <table>
          <tbody>
            <tr>
              <td style={{ color: "var(--muted-fg)", width: 130 }}>Loomy 安装</td>
              <td className="mono selectable">
                {st?.root ? (
                  st.root
                ) : (
                  <span className="err">
                    <XCircle size={12} /> 未找到
                  </span>
                )}
                {st && st.roots.length > 1 && (
                  <div className="dim" style={{ fontSize: 11.5, marginTop: 4 }}>
                    共找到 {st.roots.length} 个：{st.roots.join("  |  ")}
                  </div>
                )}
              </td>
            </tr>
            <tr>
              <td style={{ color: "var(--muted-fg)" }}>登录态文件</td>
              <td className="mono selectable">{st?.authFile ?? "-"}</td>
            </tr>
            <tr>
              <td style={{ color: "var(--muted-fg)" }}>localStorage</td>
              <td className="mono selectable">
                {st?.leveldbLog ? (
                  st.leveldbLog
                ) : (
                  <span className="warn">未找到（切换可能不生效）</span>
                )}
              </td>
            </tr>
            <tr>
              <td style={{ color: "var(--muted-fg)" }}>Loomy.exe</td>
              <td className="mono selectable">
                {st?.exe ?? <span className="dim">未找到（不影响切换，只影响"启动 Loomy"）</span>}
              </td>
            </tr>
            <tr>
              <td style={{ color: "var(--muted-fg)" }}>运行状态</td>
              <td>
                {running ? (
                  <span className="badge off">
                    运行中（{st!.runningPids.length} 个进程）
                  </span>
                ) : (
                  <span className="badge on">
                    <CheckCircle2 size={11} /> 未运行
                  </span>
                )}
              </td>
            </tr>
            <tr>
              <td style={{ color: "var(--muted-fg)" }}>数据目录</td>
              {/* 真实值，由后端返回 */}
              <td className="mono selectable">{st?.dataDir ?? "-"}</td>
            </tr>
            <tr>
              <td style={{ color: "var(--muted-fg)" }}>账号库</td>
              <td className="mono selectable">{st?.accountsFile ?? "-"}</td>
            </tr>
            <tr>
              <td style={{ color: "var(--muted-fg)" }}>探测方式</td>
              <td>
                {st?.manualPath ? (
                  <>
                    <span className="badge">手动指定</span>
                    <span className="mono" style={{ marginLeft: 8, fontSize: 11.5 }}>
                      {st.manualPath}
                    </span>
                    <button
                      className="btn sm ghost"
                      style={{ marginLeft: 8 }}
                      disabled={saving}
                      onClick={() => void clearManual()}
                    >
                      改回自动
                    </button>
                  </>
                ) : (
                  <span className="badge on">自动探测</span>
                )}
              </td>
            </tr>
          </tbody>
        </table>
      </div>

      {/* ── 手动指定（探测失败时的兜底，也可主动改）─────────────────── */}
      <div className="card">
        <h2>
          <Search size={15} /> 手动指定 Loomy 位置
          <span className="hint">
            {st?.needsManual ? "（现在需要你指定）" : "（一般不用改）"}
          </span>
        </h2>

        <div className="alert info">
          <FolderOpen size={15} className="ico" />
          <div className="body">
            选包含 <span className="mono">userData</span> 的那一层目录，通常是：
            {"\n"}
            <span className="mono selectable">C:\Users\Public\Loomy\&lt;安装ID&gt;</span>
            {"\n\n"}
            也可以直接选 <span className="mono">userData</span> 目录，或那个{" "}
            <span className="mono">auth-session.json</span> 文件 —— 三种都能识别。
          </div>
        </div>

        {/* 自动探测给的候选，点一下就填进输入框（不自动采用，需用户确认） */}
        {st && st.guessCandidates.length > 0 && (
          <div style={{ marginBottom: 12 }}>
            <div className="dim" style={{ fontSize: 12, marginBottom: 6 }}>
              探测到这些目录，可能是你要的（点一下填入）：
            </div>
            <div className="row" style={{ gap: 6 }}>
              {st.guessCandidates.map((c) => (
                <button
                  key={c}
                  className="btn sm"
                  onClick={() => setManualInput(c)}
                  title={c}
                >
                  {c}
                </button>
              ))}
            </div>
          </div>
        )}

        <div className="row">
          <input
            type="text"
            className="mono"
            placeholder="C:\Users\Public\Loomy\502d24baf4b6"
            value={manualInput}
            onChange={(e) => setManualInput(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") void saveManual(manualInput)
            }}
            style={{ flex: "1 1 320px" }}
          />
          <button className="btn" disabled={saving} onClick={() => void pickDir()}>
            浏览…
          </button>
          <button
            className="btn primary"
            disabled={saving || !manualInput.trim()}
            onClick={() => void saveManual(manualInput)}
          >
            {saving ? "校验中…" : "校验并保存"}
          </button>
        </div>
        <div className="dim" style={{ fontSize: 12, marginTop: 8 }}>
          保存前会当场校验；路径不对会明确告诉你缺什么，<strong>不会</strong>存下一个用不了的路径。
        </div>
      </div>

      {/* ── 外观 ─────────────────────────────────────────────────────── */}
      <div className="card">
        <h2>
          <Palette size={15} /> 外观
          <span className="hint">（点侧栏图标也能快速轮换）</span>
        </h2>
        <div className="theme-grid">
          {THEME_LIST.map((t) => (
            <button
              key={t.key}
              className={`theme-cell ${theme === t.key ? "on" : ""}`}
              onClick={() => onThemeChange(t.key)}
              title={t.name}
            >
              <span className="theme-swatch" style={{ background: t.color }} />
              <span className="tn">{t.name}</span>
            </button>
          ))}
        </div>
      </div>

      {/* ── 备份 ─────────────────────────────────────────────────────── */}
      <div className="card">
        <h2>
          <RotateCcw size={15} /> 备份
          <span className="hint">每次切换前自动生成，可据此撤销</span>
        </h2>
        {backups.length === 0 ? (
          <div className="empty">还没有备份 —— 第一次切换账号时会自动生成</div>
        ) : (
          <table>
            <thead>
              <tr>
                <th>时间</th>
                <th>原账号 userid</th>
                <th>localStorage</th>
                <th></th>
              </tr>
            </thead>
            <tbody>
              {backups.map((b, i) => (
                <tr key={b.name}>
                  <td className="mono">
                    {fmt(b.createdAt)}
                    {i === 0 && (
                      <span className="badge" style={{ marginLeft: 8 }}>
                        最近
                      </span>
                    )}
                  </td>
                  <td className="mono selectable">{b.userid || "-"}</td>
                  <td>{b.leveldb ? <span className="ok">已存</span> : <span className="dim">无</span>}</td>
                  <td style={{ textAlign: "right" }}>
                    <button className="btn sm ghost" disabled={busy} onClick={() => void restore(b.name)}>
                      恢复
                    </button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
        <div className="dim" style={{ marginTop: 12, fontSize: 12 }}>
          恢复时会<strong>同时</strong>还原 localStorage 里的登录态 —— 只还原
          auth-session.json 的话，下次启动 Loomy 又会被 localStorage 里的旧值覆盖回去。
        </div>
      </div>

      {/* ── 其它 ─────────────────────────────────────────────────────── */}
      <div className="card">
        <h2>
          <Trash2 size={15} /> 其它
        </h2>
        <div className="row">
          <button
            className="btn"
            onClick={async () => {
              try {
                const pids = await api.killLoomy()
                notify(
                  pids.length === 0 ? "ok" : "warn",
                  pids.length === 0 ? "Loomy 已退出" : `仍有 ${pids.length} 个进程未退出`,
                )
                await loadStatus()
              } catch (e) {
                notify("err", errorText(e))
              }
            }}
          >
            强制结束 Loomy
          </button>
          <button
            className="btn"
            onClick={async () => {
              try {
                await api.launchLoomy()
                notify("ok", "Loomy 已启动")
              } catch (e) {
                notify("err", errorText(e))
              }
            }}
          >
            启动 Loomy
          </button>
        </div>
      </div>
    </div>
  )
}
