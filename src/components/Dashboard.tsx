/**
 * 仪表盘 —— 抄 Kiro 的首页（hj01857655/kiro-account-manager）。
 *
 * # 抄了什么
 *
 * | Kiro 的首页 | 这里 |
 * |---|---|
 * | 顶部一排统计卡（总账号数 / 正常 / PRO / 配额使用率） | 账号总数 / 当前账号 / 可用积分合计 / 任务完成度 |
 * | 下面大卡片："当前 IDE 账号" + 右侧"当前 CLI 账号" | 左：当前账号详情；右：它的余额与任务进度 |
 * | 卡片内进度条 | 任务进度条 |
 *
 * 目的：**一进来就知道"我现在用哪个号、还有多少额度、任务做完没有"** ——
 * 不必逐个账号点进去。这就是 Kiro 首页的信息层级。
 */

import { useCallback, useEffect, useState } from "react"
import {
  Activity,
  AlertTriangle,
  CheckCircle2,
  Coins,
  ListChecks,
  RefreshCw,
  UserRound,
} from "lucide-react"
import { api, errorText } from "../api/tauri"
import type { AccountView, Balance, TaskProgress } from "../api/tauri"
import type { Toast } from "../App"

export default function Dashboard({
  accounts,
  activeUserid,
  onNavigate,
  busy,
}: {
  accounts: AccountView[]
  activeUserid: string | null
  onNavigate: (p: "accounts" | "import") => void
  busy: boolean
  /** 保留：后续要加"批量刷新"之类的操作时用得上，现在只有查询、不需提示 */
  notify?: (k: Toast["kind"], t: string) => void
}) {
  const current = accounts.find((a) => a.userid === activeUserid) ?? null

  const [bal, setBal] = useState<Balance | null>(null)
  const [tasks, setTasks] = useState<TaskProgress | null>(null)
  const [err, setErr] = useState("")
  const [loading, setLoading] = useState(false)

  const load = useCallback(async () => {
    if (!current) return
    setLoading(true)
    setErr("")
    try {
      const [b, t] = await Promise.all([
        api.balance(current.userid),
        api.tasks(current.userid),
      ])
      setBal(b)
      setTasks(t)
    } catch (e) {
      setErr(errorText(e))
    } finally {
      setLoading(false)
    }
  }, [current])

  useEffect(() => {
    void load()
  }, [load])

  const doneCount = tasks?.tasks.filter((t) => t.done).length ?? 0
  const taskTotal = tasks?.tasks.length ?? 0
  const pct = taskTotal > 0 ? Math.round((doneCount / taskTotal) * 100) : 0

  return (
    <div className="fade-in">
      {/* 统计卡一排 */}
      <div className="stats" style={{ marginBottom: 18 }}>
        <div className="stat">
          <div className="lbl">
            <UserRound size={13} /> 账号总数
          </div>
          <div className="val">{accounts.length}</div>
        </div>
        <div className="stat">
          <div className="lbl">
            <Activity size={13} /> 当前账号
          </div>
          <div className="val" style={{ fontSize: 16 }}>
            {current ? current.displayName : "未登录"}
          </div>
        </div>
        <div className="stat">
          <div className="lbl">
            <Coins size={13} /> 可用积分
          </div>
          <div className="val accent">
            {loading ? <span className="spin" /> : (bal?.availableBalance ?? 0).toLocaleString()}
          </div>
        </div>
        <div className="stat">
          <div className="lbl">
            <ListChecks size={13} /> 新手任务
          </div>
          <div className="val" style={{ fontSize: 18 }}>
            {loading ? <span className="spin" /> : `${doneCount} / ${taskTotal}`}
          </div>
        </div>
      </div>

      {err && (
        <div className="alert err">
          <AlertTriangle size={15} className="ico" />
          <div className="body">
            查询失败：{err}
            <br />
            <button className="btn sm" style={{ marginTop: 8 }} onClick={() => void load()}>
              重试
            </button>
          </div>
        </div>
      )}

      <div className="grid-2">
        {/* 左：当前账号 */}
        <div className="card">
          <h2>
            <UserRound size={15} /> 当前 Loomy 账号
            <span className="spacer" />
            <button className="btn sm ghost" onClick={() => onNavigate("accounts")}>
              管理
            </button>
          </h2>

          {!current ? (
            <div className="empty">
              没有检测到登录中的账号
              <div style={{ marginTop: 10 }}>
                <button className="btn primary sm" onClick={() => onNavigate("import")}>
                  导入账号
                </button>
              </div>
            </div>
          ) : (
            <>
              <div className="acct-top" style={{ marginBottom: 14 }}>
                <div
                  className="acct-avatar"
                  style={{ background: avatarColor(current.userid) }}
                >
                  {current.displayName.slice(0, 1)}
                </div>
                <div className="acct-name">
                  <div className="n">{current.displayName}</div>
                  <div className="s mono">{current.userid}</div>
                </div>
                <span className="badge on">
                  <CheckCircle2 size={11} /> 已登录
                </span>
              </div>

              <table>
                <tbody>
                  <tr>
                    <td style={{ color: "var(--muted-fg)", width: 76 }}>手机号</td>
                    <td className="mono">{current.phone || "-"}</td>
                  </tr>
                  <tr>
                    <td style={{ color: "var(--muted-fg)" }}>昵称</td>
                    <td>{current.nickname || "-"}</td>
                  </tr>
                  <tr>
                    <td style={{ color: "var(--muted-fg)" }}>session</td>
                    <td className="mono">{current.session.slice(0, 16)}…</td>
                  </tr>
                </tbody>
              </table>
            </>
          )}
        </div>

        {/* 右：额度与任务 */}
        <div className="card">
          <h2>
            <Coins size={15} /> 额度
            <span className="spacer" />
            <button className="btn sm ghost" onClick={() => void load()} disabled={loading || busy}>
              <RefreshCw size={12} /> 刷新
            </button>
          </h2>

          {!current ? (
            <div className="empty">-</div>
          ) : (
            <>
              {/* 用 .stats-2 类而不是内联 gridTemplateColumns ──
                  内联的 "1fr 1fr" 会覆盖 CSS 里的 minmax(0,1fr)，
                  于是网格项又变回 min-width:auto，长内容把列撑出容器。 */}
              <div className="stats stats-2" style={{ marginBottom: 16 }}>
                <div className="stat">
                  <div className="lbl">可用总额</div>
                  <div className="val accent" style={{ fontSize: 20 }}>
                    {loading ? <span className="spin" /> : (bal?.availableBalance ?? 0).toLocaleString()}
                  </div>
                </div>
                <div className="stat">
                  <div className="lbl">当日积分</div>
                  <div className="val" style={{ fontSize: 20 }}>
                    {loading ? <span className="spin" /> : (bal?.dailyBalance ?? 0).toLocaleString()}
                  </div>
                </div>
              </div>

              {!loading && bal && bal.lastRecord && (
                <div className="dim" style={{ fontSize: 12, marginBottom: 14 }}>
                  最近一笔：{bal.lastRecord.modelName} ·{" "}
                  {bal.lastRecord.direction === "debit" ? "消耗" : "增加"}{" "}
                  <span className="mono">{bal.lastRecord.points}</span> 分
                </div>
              )}

              <div className="row between" style={{ marginBottom: 6, fontSize: 12.5 }}>
                <span className="dim">
                  <ListChecks size={12} /> 新手任务
                </span>
                <span className="mono">
                  {doneCount}/{taskTotal} · {tasks?.earned.toLocaleString() ?? 0} 分
                </span>
              </div>
              <div className={`bar ${pct === 100 ? "ok" : ""}`}>
                <i style={{ width: `${pct}%` }} />
              </div>
            </>
          )}
        </div>
      </div>
    </div>
  )
}

/** 跟 Kiro 一样：按 userid 稳定生成一个头像色（同一个账号每次都是同色）。 */
export function avatarColor(seed: string): string {
  const palette = [
    "linear-gradient(135deg,#3b82f6,#1d4ed8)",
    "linear-gradient(135deg,#8b5cf6,#6d28d9)",
    "linear-gradient(135deg,#06b6d4,#0e7490)",
    "linear-gradient(135deg,#10b981,#047857)",
    "linear-gradient(135deg,#f59e0b,#b45309)",
    "linear-gradient(135deg,#ef4444,#b91c1c)",
    "linear-gradient(135deg,#ec4899,#be185d)",
    "linear-gradient(135deg,#6366f1,#4338ca)",
  ]
  let h = 0
  for (let i = 0; i < seed.length; i++) h = (h * 31 + seed.charCodeAt(i)) >>> 0
  return palette[h % palette.length]
}
