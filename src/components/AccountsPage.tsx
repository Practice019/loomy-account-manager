/**
 * 账号管理 —— 抄 Kiro 的账号卡片网格。
 *
 * # 抄了什么
 *
 * | Kiro 的账号卡片 | 这里 |
 * |---|---|
 * | 头像块（按名字取色）+ 名称 + 副标题 | 同样的 `.acct-top` / `.acct-avatar` |
 * | 标签行（`KIRO FREE` / `github`） | 手机号 / 昵称 标签 |
 * | 进度条（使用量 2%） | 不必做（Loomy 没有"账号配额上限"的概念） |
 * | 底部一排图标操作（登录/查看/刷新/编辑/删除） | 切换 / 复制 session / 删除 |
 * | 右上角开关 + 状态徽标（正常/封顶） | 当前 / 未登录 徽标 |
 *
 * 卡片网格 + 右侧详情抽屉的布局也是抄的（它用 Modal，我用右侧面板 ——
 * 桌面宽屏下抽屉比弹窗少一次"打开/关闭"）。
 */

import { useMemo, useState } from "react"
import {
  CheckCircle2,
  Copy,
  RefreshCw,
  Search,
  Trash2,
  Unplug,
  Wallet,
} from "lucide-react"
import { api, errorText } from "../api/tauri"
import type { AccountView, BalanceRow } from "../api/tauri"
import type { Toast } from "../App"
import { avatarColor } from "./Dashboard"
import AccountDetail from "./AccountDetail"

export default function AccountsPage({
  accounts,
  activeUserid,
  busy,
  onSwitch,
  onRefresh,
  notify,
}: {
  accounts: AccountView[]
  activeUserid: string | null
  busy: boolean
  onSwitch: (userid: string, restart: boolean) => void
  onRefresh: () => void | Promise<void>
  notify: (k: Toast["kind"], t: string) => void
}) {
  const [q, setQ] = useState("")
  const [balances, setBalances] = useState<Record<string, BalanceRow>>({})
  const [fetchingBal, setFetchingBal] = useState(false)

  // ── 选中项：用户点击优先，否则回落到"当前登录的账号"，再否则第一个 ──
  //
  // # 为什么不能写成 `useState(activeUserid)`
  //
  // 那是**第一版**的写法，有个真实缺陷：`useState` 的初值只在**挂载那一刻**
  // 取一次，而那时账号数据还在异步加载中（`activeUserid` 是 `null`）。
  // 于是 `selected` 永远停在 `null`，**右侧详情面板一直不显示** ——
  // 界面看起来像"右边那块没了"，用户必须手动点一张卡片才出现。
  //
  // 正确做法：把"用户点过的"和"算出来的默认值"分开。用 `null` 表示
  // "用户还没选"，实际显示时再逐级回落。这样数据加载完会自动补上默认选中。
  const [picked, setPicked] = useState<string | null>(null)

  const selected = useMemo(() => {
    // ① 用户点过的（且还在列表里）
    if (picked && accounts.some((a) => a.userid === picked)) return picked
    // ② 当前登录的账号
    if (activeUserid && accounts.some((a) => a.userid === activeUserid)) return activeUserid
    // ③ 第一个
    return accounts[0]?.userid ?? null
  }, [picked, accounts, activeUserid])

  const filtered = useMemo(() => {
    const s = q.trim().toLowerCase()
    if (!s) return accounts
    return accounts.filter(
      (a) =>
        a.userid.toLowerCase().includes(s) ||
        a.phone.toLowerCase().includes(s) ||
        a.nickname.toLowerCase().includes(s) ||
        a.displayName.toLowerCase().includes(s),
    )
  }, [accounts, q])

  const current = accounts.find((a) => a.userid === selected) ?? null

  /** 批量查余额（一次 IPC，避免逐个往返）。 */
  const loadBalances = async () => {
    if (accounts.length === 0) return
    setFetchingBal(true)
    try {
      const rows = await api.balances(accounts.map((a) => a.userid))
      const map: Record<string, BalanceRow> = {}
      for (const r of rows) map[r.userid] = r
      setBalances(map)
    } catch (e) {
      notify("err", "查余额失败：" + errorText(e))
    } finally {
      setFetchingBal(false)
    }
  }

  const del = async (a: AccountView) => {
    if (!confirm(`删除账号 ${a.displayName}？\n\n（只是从本工具的库里移除，不影响 Loomy 的登录状态）`))
      return
    try {
      await api.deleteAccount(a.userid)
      notify("ok", "已删除")
      if (selected === a.userid) setPicked(null)
      await onRefresh()
    } catch (e) {
      notify("err", errorText(e))
    }
  }

  return (
    <div className="fade-in">
      {/* 工具条：搜索 + 批量查余额 */}
      <div className="row" style={{ marginBottom: 14 }}>
        <div style={{ position: "relative", flex: "1 1 240px", maxWidth: 320 }}>
          <Search
            size={14}
            style={{
              position: "absolute",
              left: 10,
              top: "50%",
              transform: "translateY(-50%)",
              color: "var(--muted-fg)",
              pointerEvents: "none",
            }}
          />
          <input
            type="text"
            placeholder="搜索手机号 / userid / 昵称"
            value={q}
            onChange={(e) => setQ(e.target.value)}
            style={{ paddingLeft: 31 }}
          />
        </div>
        <button className="btn" onClick={() => void loadBalances()} disabled={fetchingBal}>
          {fetchingBal ? <span className="spin" /> : <Wallet size={13} />}
          查询全部余额
        </button>
        <button className="btn" onClick={() => void onRefresh()} disabled={busy}>
          <RefreshCw size={13} /> 刷新
        </button>
        <div className="spacer" />
        <span className="dim" style={{ fontSize: 12 }}>
          共 {accounts.length} 个
          {q && ` · 匹配 ${filtered.length} 个`}
        </span>
      </div>

      <div className="acct-split">
        {/* 卡片网格 */}
        <div style={{ flex: 1, minWidth: 0 }}>
          {filtered.length === 0 ? (
            <div className="card">
              <div className="empty">
                {accounts.length === 0 ? "还没有账号，去「导入账号」加一个" : "没有匹配的账号"}
              </div>
            </div>
          ) : (
            <div className="acct-grid">
              {filtered.map((a) => {
                const isActive = a.userid === activeUserid
                const b = balances[a.userid]
                return (
                  <div
                    key={a.userid}
                    className={`acct-card ${isActive ? "active" : ""} ${selected === a.userid ? "selected" : ""}`}
                    onClick={() => setPicked(a.userid)}
                  >
                    <div className="acct-top">
                      <div className="acct-avatar" style={{ background: avatarColor(a.userid) }}>
                        {a.displayName.slice(0, 1)}
                      </div>
                      <div className="acct-name">
                        <div className="n">{a.displayName}</div>
                        <div className="s mono">{a.phone || a.userid}</div>
                      </div>
                      {isActive ? (
                        <span className="badge on">
                          <CheckCircle2 size={11} /> 当前
                        </span>
                      ) : (
                        <span className="badge off">
                          <Unplug size={11} /> 未登录
                        </span>
                      )}
                    </div>

                    <div className="acct-tags">
                      {a.nickname && <span className="tag">{a.nickname}</span>}
                      {a.note && <span className="tag dim">{a.note}</span>}
                      {b && !b.error && (
                        <span className="tag ok">
                          <Wallet size={10} /> {b.available.toLocaleString()} 分
                        </span>
                      )}
                      {b && b.error && <span className="tag dim">余额查询失败</span>}
                    </div>

                    <div className="acct-actions" onClick={(e) => e.stopPropagation()}>
                      {!isActive && (
                        <button
                          className="btn sm"
                          disabled={busy}
                          onClick={() => onSwitch(a.userid, true)}
                        >
                          切换
                        </button>
                      )}
                      <button
                        className="btn sm ghost"
                        title="复制 session"
                        onClick={() => {
                          void navigator.clipboard.writeText(a.session)
                          notify("ok", "session 已复制")
                        }}
                      >
                        <Copy size={12} />
                      </button>
                      <div className="spacer" />
                      <button
                        className="btn sm ghost danger"
                        title="从库里删除"
                        onClick={() => void del(a)}
                      >
                        <Trash2 size={12} />
                      </button>
                    </div>
                  </div>
                )
              })}
            </div>
          )}
        </div>

        {/* 右侧详情（宽屏并排；窄屏落到网格下方，见 .acct-split 的 media query） */}
        {current && (
          <div className="acct-detail-aside">
            <AccountDetail
              account={current}
              busy={busy}
              onSwitch={onSwitch}
              onRefresh={onRefresh}
              notify={notify}
            />
          </div>
        )}
      </div>
    </div>
  )
}
