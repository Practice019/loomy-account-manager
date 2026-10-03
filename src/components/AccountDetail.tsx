/**
 * 账号详情（右侧面板）—— 抄 Kiro 的 AccountDetailModal 的信息分组，
 * 但用**侧栏面板**而不是弹窗：桌面宽屏下省掉一次"打开/关闭"。
 *
 * 三组信息：额度 / 新手任务 / 邀请码。每组都有加载中、失败、空三态
 *（Kiro 那边这点做得对：不是只有成功路径）。
 */

import { useCallback, useEffect, useState } from "react"
import {
  AlertTriangle,
  Check,
  Coins,
  Copy,
  Download,
  Gift,
  ListChecks,
  Play,
  Power,
  RefreshCw,
} from "lucide-react"
import type { AccountView, Balance, InviteCode, TaskProgress } from "../api/tauri"
import { api, errorText } from "../api/tauri"
import type { Toast } from "../App"
import { avatarColor } from "./Dashboard"

type Tab = "balance" | "tasks" | "invite"

export default function AccountDetail({
  account,
  busy,
  onSwitch,
  onRefresh,
  notify,
}: {
  account: AccountView
  busy: boolean
  onSwitch: (userid: string, restart: boolean) => void
  onRefresh: () => void | Promise<void>
  notify: (k: Toast["kind"], t: string) => void
}) {
  const [tab, setTab] = useState<Tab>("balance")

  return (
    <>
      <div className="card">
        {/* 头部：头像 + 名称 + 状态（与卡片网格一致的语言） */}
        <div className="acct-top" style={{ marginBottom: 14 }}>
          <div className="acct-avatar" style={{ background: avatarColor(account.userid) }}>
            {account.displayName.slice(0, 1)}
          </div>
          <div className="acct-name">
            <div className="n">{account.displayName}</div>
            <div className="s mono">{account.userid}</div>
          </div>
          {account.active && <span className="badge on">当前</span>}
        </div>

        <table style={{ marginBottom: 14 }}>
          <tbody>
            <tr>
              <td style={{ color: "var(--muted-fg)", width: 62 }}>手机号</td>
              <td className="mono">{account.phone || "-"}</td>
            </tr>
            <tr>
              <td style={{ color: "var(--muted-fg)" }}>昵称</td>
              <td>{account.nickname || "-"}</td>
            </tr>
            <tr>
              <td style={{ color: "var(--muted-fg)" }}>session</td>
              <td className="mono">
                {account.session.slice(0, 14)}…
                <button
                  className="btn sm ghost"
                  style={{ marginLeft: 6 }}
                  onClick={() => {
                    void navigator.clipboard.writeText(account.session)
                    notify("ok", "session 已复制")
                  }}
                >
                  <Copy size={11} />
                </button>
              </td>
            </tr>
            {account.note && (
              <tr>
                <td style={{ color: "var(--muted-fg)" }}>备注</td>
                <td>{account.note}</td>
              </tr>
            )}
          </tbody>
        </table>

        <div className="row">
          {account.active ? (
            <button className="btn primary" disabled>
              <Check size={13} /> 已是当前账号
            </button>
          ) : (
            <>
              <button
                className="btn primary"
                disabled={busy}
                onClick={() => onSwitch(account.userid, true)}
              >
                <Power size={13} /> 切换并重启 Loomy
              </button>
              <button className="btn" disabled={busy} onClick={() => onSwitch(account.userid, false)}>
                只切文件
              </button>
            </>
          )}
          <button
            className="btn ghost"
            disabled={busy}
            title="复制 token JSON（可直接发给别人）"
            onClick={async () => {
              try {
                const txt = await api.exportAccount(account.userid)
                await navigator.clipboard.writeText(txt)
                notify("ok", "token 已复制到剪贴板")
              } catch (e) {
                notify("err", errorText(e))
              }
            }}
          >
            <Download size={13} />
          </button>
        </div>
      </div>

      {/* 子标签 */}
      <div className="row" style={{ marginBottom: 12, gap: 6 }}>
        {(
          [
            ["balance", "额度", Coins],
            ["tasks", "新手任务", ListChecks],
            ["invite", "邀请码", Gift],
          ] as const
        ).map(([id, label, Icon]) => (
          <button
            key={id}
            className={`btn ${tab === id ? "primary" : "ghost"}`}
            onClick={() => setTab(id)}
          >
            <Icon size={13} /> {label}
          </button>
        ))}
      </div>

      {tab === "balance" && <BalancePanel userid={account.userid} />}
      {tab === "tasks" && (
        <TasksPanel userid={account.userid} notify={notify} onChanged={onRefresh} />
      )}
      {tab === "invite" && <InvitePanel userid={account.userid} notify={notify} />}
    </>
  )
}

/** 三态包装：加载 / 失败（可重试）/ 内容。三个面板共用。 */
function Panel({
  loading,
  err,
  onRetry,
  children,
}: {
  loading: boolean
  err: string
  onRetry: () => void
  children: React.ReactNode
}) {
  if (loading) {
    return (
      <div className="card">
        <div className="empty">
          查询中… <span className="spin" />
        </div>
      </div>
    )
  }
  if (err) {
    return (
      <div className="card">
        <div className="alert err" style={{ marginBottom: 0 }}>
          <AlertTriangle size={15} className="ico" />
          <div className="body">
            查询失败：{err}
            <div style={{ marginTop: 8 }}>
              <button className="btn sm" onClick={onRetry}>
                重试
              </button>
            </div>
          </div>
        </div>
      </div>
    )
  }
  return <>{children}</>
}

// ── 额度 ────────────────────────────────────────────────────────────────────

function BalancePanel({ userid }: { userid: string }) {
  const [b, setB] = useState<Balance | null>(null)
  const [err, setErr] = useState("")
  const [loading, setLoading] = useState(true)

  const load = useCallback(async () => {
    setLoading(true)
    setErr("")
    try {
      setB(await api.balance(userid))
    } catch (e) {
      setErr(errorText(e))
    } finally {
      setLoading(false)
    }
  }, [userid])

  useEffect(() => {
    void load()
  }, [load])

  return (
    <Panel loading={loading} err={err} onRetry={() => void load()}>
      <div className="card">
        <h2>
          <Coins size={15} /> 额度
          <span className="spacer" />
          <button className="btn sm ghost" onClick={() => void load()}>
            <RefreshCw size={11} />
          </button>
        </h2>
        <div className="stats">
          <div className="stat">
            <div className="lbl">可用总额</div>
            <div className="val accent">{(b?.availableBalance ?? 0).toLocaleString()}</div>
          </div>
          <div className="stat">
            <div className="lbl">永久积分</div>
            <div className="val" style={{ fontSize: 20 }}>
              {(b?.balance ?? 0).toLocaleString()}
            </div>
          </div>
          <div className="stat">
            <div className="lbl">当日积分</div>
            <div className="val" style={{ fontSize: 20 }}>
              {(b?.dailyBalance ?? 0).toLocaleString()}
            </div>
          </div>
        </div>
        {b?.lastRecord && (
          <div className="dim" style={{ marginTop: 12, fontSize: 12 }}>
            最近一笔：{b.lastRecord.modelName} ·{" "}
            {b.lastRecord.direction === "debit" ? "消耗" : "增加"}{" "}
            <span className="mono">{b.lastRecord.points}</span> 分
            {b.lastRecord.createdAt > 0 && (
              <>
                {" · "}
                {new Date(b.lastRecord.createdAt * 1000).toLocaleString("zh-CN", {
                  month: "numeric",
                  day: "numeric",
                  hour: "2-digit",
                  minute: "2-digit",
                })}
              </>
            )}
          </div>
        )}
      </div>
    </Panel>
  )
}

// ── 新手任务 ────────────────────────────────────────────────────────────────

function TasksPanel({
  userid,
  notify,
  onChanged,
}: {
  userid: string
  notify: (k: Toast["kind"], t: string) => void
  onChanged: () => void | Promise<void>
}) {
  const [p, setP] = useState<TaskProgress | null>(null)
  const [err, setErr] = useState("")
  const [loading, setLoading] = useState(true)
  const [running, setRunning] = useState(false)

  const load = useCallback(async () => {
    setLoading(true)
    setErr("")
    try {
      setP(await api.tasks(userid))
    } catch (e) {
      setErr(errorText(e))
    } finally {
      setLoading(false)
    }
  }, [userid])

  useEffect(() => {
    void load()
  }, [load])

  const runAll = useCallback(async () => {
    setRunning(true)
    try {
      const results = await api.completeAllTasks(userid)
      if (results.length === 0) {
        notify("info", "没有未完成的任务了")
      } else {
        const ok = results.filter((r) => r.ok)
        const bad = results.filter((r) => !r.ok)
        const lines = [`完成 ${ok.length}/${results.length} 个任务`]
        if (ok.length) lines.push(`获得 ${ok.reduce((s, r) => s + r.points, 0)} 积分`)
        if (bad.length) {
          lines.push("", "未完成：")
          for (const r of bad) lines.push(`  ${r.label}：${r.error}`)
        }
        notify(bad.length === 0 ? "ok" : "warn", lines.join("\n"))
      }
      await load()
      await onChanged()
    } catch (e) {
      notify("err", "执行失败：" + errorText(e))
    } finally {
      setRunning(false)
    }
  }, [userid, notify, load, onChanged])

  const undone = p?.tasks.filter((t) => !t.done).length ?? 0
  const pct = p && p.tasks.length ? Math.round(((p.tasks.length - undone) / p.tasks.length) * 100) : 0

  return (
    <Panel loading={loading} err={err} onRetry={() => void load()}>
      <div className="card">
        <h2>
          <ListChecks size={15} /> 新手任务
          <span className="hint">
            {p?.earned.toLocaleString() ?? 0} / {p?.total.toLocaleString() ?? 0}
          </span>
        </h2>

        <div className={`bar ${pct === 100 ? "ok" : ""}`} style={{ marginBottom: 12 }}>
          <i style={{ width: `${pct}%` }} />
        </div>

        {p?.tasks.map((t) => (
          <div className="task-row" key={t.key}>
            <span className={`mk ${t.done ? "ok" : "dim"}`}>{t.done ? "✓" : "○"}</span>
            <span className="nm">{t.label}</span>
            <span className="pt">+{t.points}</span>
          </div>
        ))}

        <div className="row" style={{ marginTop: 14 }}>
          <button
            className="btn primary"
            disabled={running || undone === 0}
            onClick={() => void runAll()}
          >
            {running ? (
              <>
                <span className="spin" /> 执行中…
              </>
            ) : undone === 0 ? (
              <>
                <Check size={13} /> 全部已完成
              </>
            ) : (
              <>
                <Play size={13} /> 一键完成剩余 {undone} 个
              </>
            )}
          </button>
          <button className="btn ghost" disabled={running} onClick={() => void load()}>
            <RefreshCw size={12} />
          </button>
        </div>
      </div>
    </Panel>
  )
}

// ── 邀请码 ──────────────────────────────────────────────────────────────────

function InvitePanel({
  userid,
  notify,
}: {
  userid: string
  notify: (k: Toast["kind"], t: string) => void
}) {
  const [act, setAct] = useState<{ activated: boolean; appliedCode: string } | null>(null)
  const [mine, setMine] = useState<InviteCode[]>([])
  const [code, setCode] = useState("")
  const [err, setErr] = useState("")
  const [loading, setLoading] = useState(true)
  const [binding, setBinding] = useState(false)

  const load = useCallback(async () => {
    setLoading(true)
    setErr("")
    try {
      const [a, m] = await Promise.all([api.activation(userid), api.myInvites(userid)])
      setAct(a)
      setMine(m)
    } catch (e) {
      setErr(errorText(e))
    } finally {
      setLoading(false)
    }
  }, [userid])

  useEffect(() => {
    void load()
  }, [load])

  const bind = useCallback(async () => {
    const c = code.trim().toUpperCase()
    if (!c) return
    setBinding(true)
    try {
      await api.bindInvite(userid, c)
      notify("ok", `已绑定邀请码 ${c}`)
      setCode("")
      await load()
    } catch (e) {
      // 上游的 desc 是给用户看的（"邀请码不存在"/"不能绑定自己生成的码"），原样显示
      notify("err", errorText(e))
    } finally {
      setBinding(false)
    }
  }, [code, userid, notify, load])

  return (
    <Panel loading={loading} err={err} onRetry={() => void load()}>
      <div className="card">
        <h2>
          <Gift size={15} /> 激活状态
        </h2>
        {act?.activated ? (
          <div className="alert ok" style={{ marginBottom: 0 }}>
            <Check size={15} className="ico" />
            <div className="body">
              已激活 —— 使用了邀请码 <strong className="mono">{act.appliedCode}</strong>
            </div>
          </div>
        ) : (
          <>
            <div className="alert warn">
              <AlertTriangle size={15} className="ico" />
              <div className="body">未激活。填入一个别人的邀请码即可激活（双方都有积分奖励）</div>
            </div>
            <div className="row">
              <input
                type="text"
                placeholder="邀请码，例如 AB12CD"
                value={code}
                maxLength={12}
                onChange={(e) => setCode(e.target.value.toUpperCase())}
                onKeyDown={(e) => {
                  if (e.key === "Enter") void bind()
                }}
                style={{ maxWidth: 170 }}
                className="mono"
              />
              <button className="btn primary" disabled={binding || !code.trim()} onClick={() => void bind()}>
                {binding ? "绑定中…" : "绑定"}
              </button>
            </div>
          </>
        )}
      </div>

      <div className="card">
        <h2>
          我生成的邀请码 <span className="hint">别人用了我也拿积分</span>
        </h2>
        {mine.length === 0 ? (
          <div className="empty" style={{ padding: 14 }}>
            还没有生成过邀请码
          </div>
        ) : (
          <table>
            <thead>
              <tr>
                <th>邀请码</th>
                <th>已用</th>
                <th>状态</th>
                <th></th>
              </tr>
            </thead>
            <tbody>
              {mine.map((c) => (
                <tr key={c.code}>
                  <td className="mono">{c.code || "(空)"}</td>
                  <td>
                    {c.usedCount}/{c.maxUses}
                  </td>
                  <td>
                    {/* 上游实际给的是 "active" / "exhausted"（实测），
                        不是 "available"。第一版判 "available" → 全部显示"已用完"。
                        这里改成"非 active 即不可用"，对未知值也能给出合理显示。 */}
                    {c.status === "active" ? (
                      <span className="ok">可用</span>
                    ) : (
                      <span className="dim">已用完</span>
                    )}
                  </td>
                  <td style={{ textAlign: "right" }}>
                    <button
                      className="btn sm ghost"
                      disabled={!c.code}
                      onClick={() => {
                        void navigator.clipboard.writeText(c.code)
                        notify("ok", "邀请码已复制")
                      }}
                    >
                      <Copy size={11} />
                    </button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </div>
    </Panel>
  )
}
