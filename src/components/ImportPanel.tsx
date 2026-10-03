import { useState } from "react"
import { FileSearch, Info } from "lucide-react"
import { open } from "@tauri-apps/plugin-dialog"
import { api, errorText } from "../api/tauri"
import type { Toast } from "../App"

export default function ImportPanel({
  notify,
  onDone,
}: {
  notify: (k: Toast["kind"], t: string) => void
  onDone: () => void | Promise<void>
}) {
  const [text, setText] = useState("")
  const [busy, setBusy] = useState(false)
  const [lastReport, setLastReport] = useState<string>("")

  const reportLines = (r: { added: number; updated: number; skipped: string[]; total: number }) => {
    const lines = [`新增 ${r.added} 个，更新 ${r.updated} 个，库中共 ${r.total} 个`]
    if (r.skipped.length) {
      lines.push("")
      lines.push(`跳过 ${r.skipped.length} 个：`)
      lines.push(...r.skipped.slice(0, 8).map((s) => "  " + s))
      if (r.skipped.length > 8) lines.push(`  …（还有 ${r.skipped.length - 8} 个）`)
    }
    return lines.join("\n")
  }

  const doImportText = async () => {
    if (!text.trim()) {
      notify("warn", "先贴一份 token 内容")
      return
    }
    setBusy(true)
    try {
      const r = await api.importText(text)
      notify(r.added > 0 ? "ok" : "info", reportLines(r))
      setLastReport(reportLines(r))
      setText("")
      await onDone()
    } catch (e) {
      notify("err", errorText(e))
    } finally {
      setBusy(false)
    }
  }

  const doImportDir = async () => {
    let dir: string | null = null
    try {
      const picked = await open({ directory: true, multiple: false, title: "选择账号目录" })
      if (typeof picked === "string") dir = picked
    } catch {
      // 有些环境没有 dialog 权限 —— 退化成手动输入
      dir = prompt("输入目录路径（含 *.json 或 auths/ 子目录）") ?? null
    }
    if (!dir) return

    setBusy(true)
    try {
      const r = await api.importDir(dir)
      notify(r.added > 0 ? "ok" : "info", reportLines(r))
      setLastReport(reportLines(r))
      await onDone()
    } catch (e) {
      notify("err", errorText(e))
    } finally {
      setBusy(false)
    }
  }

  return (
    <div className="fade-in">
      <div className="card">
        <h2>
          粘贴导入 <span className="hint">把 Loomy 的登录态整份贴进来</span>
        </h2>

        <div className="alert info">
          <Info size={15} className="ico" />
          <div className="body">
            从哪儿拿？Loomy 桌面端的登录态在这个文件里，把内容整个贴过来即可：
            {"\n"}
            <span className="mono selectable">
              C:\Users\Public\Loomy\&lt;安装ID&gt;\userData\auth-session.json
            </span>
            {"\n\n"}
            支持单个 JSON、JSON 数组，也兼容别的字段名（uid / userId）。
          </div>
        </div>

        <textarea
          rows={10}
          value={text}
          onChange={(e) => setText(e.target.value)}
          placeholder={`{
  "session": "0123456789abcdef0123456789abcdef",
  "userid": "260101000000000001",
  "phone": "150****0000",
  "nickname": "NK0000000"
}`}
        />

        <div className="row" style={{ marginTop: 12 }}>
          <button className="btn primary" disabled={busy} onClick={() => void doImportText()}>
            {busy ? (
              <>
                <span className="spin" /> 导入中…
              </>
            ) : (
              "导入"
            )}
          </button>
          <button className="btn ghost" disabled={busy} onClick={() => setText("")}>
            清空
          </button>
          <div className="spacer" />
          <button className="btn" disabled={busy} onClick={() => void doImportDir()}>
            <FileSearch size={13} /> 从目录批量导入…
          </button>
        </div>
      </div>

      {lastReport && (
        <div className="card">
          <h2>上次导入结果</h2>
          <pre
            className="selectable"
            style={{
              margin: 0,
              fontFamily: "ui-monospace, Consolas, monospace",
              fontSize: 12,
              whiteSpace: "pre-wrap",
              color: "var(--muted-fg)",
            }}
          >
            {lastReport}
          </pre>
        </div>
      )}
    </div>
  )
}
