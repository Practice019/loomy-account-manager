/**
 * Tauri 命令的类型化封装。
 *
 * # 为什么要有这一层
 *
 * 前端的 `invoke("cmd_name", args)` 是**字符串 + 无类型**的：命令名写错、
 * 参数名写错、返回类型猜错，TypeScript 都拦不住，只在运行时报一句
 * 含糊的错（"command not found" 或 undefined）。
 *
 * 所以这里给每个后端命令一个**具名函数 + 显式类型**：
 * 命令名只在这一个文件里出现，改后端时类型错误会直接指向调用点。
 */
import { invoke } from "@tauri-apps/api/core"

/** 后端返回的错误形状（见 src-tauri/src/core/error.rs 的 Serialize 实现）。 */
export interface BackendError {
  kind:
    | "io"
    | "json"
    | "http"
    | "notFound"
    | "loomyNotFound"
    | "loomyRunning"
    | "upstream"
    | "invalid"
    | "other"
  message: string
  detail?: { pids?: number[]; code?: string; desc?: string } | null
}

/** 把 unknown 收成 BackendError。 */
export function asBackendError(e: unknown): BackendError {
  if (e && typeof e === "object" && "message" in e) {
    const o = e as Record<string, unknown>
    return {
      kind: (o.kind as BackendError["kind"]) ?? "other",
      message: String(o.message ?? e),
      detail: (o.detail as BackendError["detail"]) ?? null,
    }
  }
  return { kind: "other", message: String(e) }
}

/** 拿错误里适合展示给用户的那段文本。 */
export function errorText(e: unknown): string {
  return asBackendError(e).message
}

// ── 数据形状（与 Rust 侧的 serde 结构一一对应）─────────────────────────────

export interface Account {
  userid: string
  session: string
  phone: string
  nickname: string
  name: string
  note: string
  source: string
  importedAt: number
}

export interface AccountView extends Account {
  active: boolean
  displayName: string
}

export interface ImportReport {
  added: number
  updated: number
  skipped: string[]
  total: number
}

export interface ApplyReport {
  jsonWritten: boolean
  opencodeFiles: string[]
  leveldbLogs: string[]
  warnings: string[]
}

export interface SwitchPrecheck {
  root: string | null
  runningPids: number[]
  canSwitchDirectly: boolean
  activeUserid: string | null
}

export interface SwitchReport {
  userid: string
  session: string
  backup: string
  applied: ApplyReport
  restarted: boolean
}

export interface BackupMeta {
  name: string
  createdAt: number
  userid: string
  session: string
  leveldb: unknown | null
}

export interface LastRecord {
  modelName: string
  /** "debit"（扣）/ "credit"（加） */
  direction: string
  points: number
  /** Unix **秒**（上游给的不是毫秒） */
  createdAt: number
  description: string
}

export interface Balance {
  balance: number
  dailyBalance: number
  availableBalance: number
  total: number
  lastRecord: LastRecord | null
}

export interface BalanceRow {
  userid: string
  available: number
  daily: number
  balance: number
  error: string
}

export interface TaskItem {
  key: string
  label: string
  points: number
  done: boolean
}

export interface TaskProgress {
  tasks: TaskItem[]
  earned: number
  total: number
}

export interface TaskResult {
  key: string
  label: string
  ok: boolean
  points: number
  error: string
}

export interface ActivationState {
  activated: boolean
  appliedCode: string
}

export interface InviteCode {
  code: string
  usedCount: number
  maxUses: number
  status: string
}

/** 本机 Loomy 环境的真实状态（由后端采集，不是前端猜的）。 */
export interface SystemStatus {
  found: boolean
  roots: string[]
  root: string | null
  authFile: string | null
  leveldbDirs: string[]
  leveldbLog: string | null
  exe: string | null
  runningPids: number[]
  dataDir: string
  accountsFile: string
  manualPath: string | null
  guessCandidates: string[]
  needsManual: boolean
  reason: string
}

// ── 命令封装 ──────────────────────────────────────────────────────────────

export const api = {
  // 账号
  listAccounts: () => invoke<AccountView[]>("list_accounts"),
  activeAccount: () => invoke<Account | null>("active_account"),
  importText: (text: string) => invoke<ImportReport>("import_text", { text }),
  importDir: (dir: string) => invoke<ImportReport>("import_dir", { dir }),
  deleteAccount: (userid: string) => invoke<number>("delete_account", { userid }),
  updateNote: (userid: string, note: string, name?: string) =>
    invoke<void>("update_account_note", { userid, note, name: name ?? null }),
  exportAccount: (userid: string) => invoke<string>("export_account", { userid }),

  // 切换
  precheck: () => invoke<SwitchPrecheck>("precheck_switch"),
  switchAccount: (userid: string, killRunning: boolean, restartAfter: boolean) =>
    invoke<SwitchReport>("switch_account", { userid, killRunning, restartAfter }),
  restoreBackup: (name?: string) =>
    invoke<SwitchReport>("restore_backup", { name: name ?? null }),
  listBackups: () => invoke<BackupMeta[]>("list_backups"),
  killLoomy: () => invoke<number[]>("kill_loomy"),
  launchLoomy: () => invoke<boolean>("launch_loomy"),

  // 元数据
  balance: (userid: string) => invoke<Balance>("fetch_balance", { userid }),
  balances: (userids: string[]) => invoke<BalanceRow[]>("fetch_balances", { userids }),
  tasks: (userid: string) => invoke<TaskProgress>("fetch_tasks", { userid }),
  completeAllTasks: (userid: string) => invoke<TaskResult[]>("complete_all_tasks", { userid }),
  activation: (userid: string) => invoke<ActivationState>("fetch_activation", { userid }),
  bindInvite: (userid: string, code: string) => invoke<void>("bind_invite", { userid, code }),
  myInvites: (userid: string) => invoke<InviteCode[]>("fetch_my_invites", { userid }),
  claimFirstLogin: (userid: string) => invoke<void>("claim_first_login", { userid }),

  // 系统状态（真实路径 / 手动指定 Loomy 位置）
  systemStatus: () => invoke<SystemStatus>("system_status"),
  setLoomyPath: (path: string) => invoke<string>("set_loomy_path", { path }),
  clearLoomyPath: () => invoke<void>("clear_loomy_path"),
  testLoomyPath: (path: string) => invoke<string | null>("test_loomy_path", { path }),
}
