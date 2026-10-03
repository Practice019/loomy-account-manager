/**
 * 布局测量台（开发工具，不参与发布构建）。
 *
 * # 它解决什么问题
 *
 * "右边被截断"这类布局问题**看截图不可靠**：内容溢出到窗口外时，截图里
 * 只是"那块没画完"，很容易误判成截图没截全。而
 * `scrollWidth > clientWidth` 是确定的证据。
 *
 * 这个文件就是当时抓到缺陷的工具：`Dashboard.tsx` 里内联的
 * `gridTemplateColumns: "1fr 1fr"` 覆盖了 CSS 的 `minmax(0, 1fr)`，
 * 网格项因此退回默认 `min-width: auto`，被长内容撑破容器。
 *
 * # 怎么用
 *
 * ```bash
 * bun run dev            # 起 vite
 * # 浏览器打开 http://localhost:1420/measure.html
 * ```
 *
 * 页面上会列出每一页的横向溢出元素；加 `?nooverlay=1` 可只看界面（截图用），
 * 加 `&page=accounts` 可直接停到某一页。
 *
 * # ⚠ 数据一律用假值
 *
 * 这里**不能**放真实账号（手机号 / userid / session）—— 本项目是分发出去的，
 * 真数据进了仓库就是隐私泄露。下面的值全部是构造的，只要求"长度与真实值
 * 一致"（溢出往往由最长的那个串触发，长度对了就能复现）。
 */

import { render } from "./measureMount"

/** 构造一个看起来像真实 session 的假串（32 位十六进制）。 */
const fakeSession = (seed: number) =>
  Array.from({ length: 32 }, (_, i) => "0123456789abcdef"[(seed * 7 + i * 3) % 16]).join("")

/** 构造一个长度与真实 userid 一致的假串（18 位数字）。 */
const fakeUserid = (seed: number) =>
  seed === 0
    ? "260101000000000001"
    : `26${String(seed).padStart(2, "0")}${String(10_0000_0000_0000 + seed * 137).slice(0, 12)}`

/** 构造一个脱敏样式的假手机号（`150****3411`）。 */
const fakePhone = (seed: number) => {
  const tail = String(1000 + seed * 137).slice(-4)
  return `${String(130 + seed * 7).slice(0, 3)}****${tail}`
}

const ACCOUNTS = Array.from({ length: 6 }, (_, i) => {
  const userid = fakeUserid(i)
  const phone = fakePhone(i)
  return {
    userid,
    session: fakeSession(i + 1),
    phone,
    nickname: `NK${phone.slice(-4)}`,
    name: "",
    note: i === 2 ? "备用号" : "",
    source: "paste",
    importedAt: i + 1,
    active: i === 0,
    displayName: i === 0 || i === 3 ? phone : `NK${phone.slice(-4)}`,
  }
})

const MOCK: Record<string, unknown> = {
  list_accounts: ACCOUNTS,
  active_account: ACCOUNTS[0],
  precheck_switch: {
    root: "C:\\Users\\Public\\Loomy\\000000000000",
    runningPids: [],
    canSwitchDirectly: true,
    activeUserid: ACCOUNTS[0].userid,
  },
  list_backups: [
    {
      name: "2026-01-01T00-00-00",
      createdAt: 1767225600000,
      userid: ACCOUNTS[0].userid,
      session: fakeSession(1),
      leveldb: { phone: ACCOUNTS[0].phone },
    },
  ],
  fetch_balance: {
    balance: 9675,
    dailyBalance: 4094,
    availableBalance: 13769,
    total: 1799,
    lastRecord: {
      modelName: "GLM-5.3-Flash",
      direction: "debit",
      points: 34,
      createdAt: 1791022091,
      description: "模型调用扣分",
    },
  },
  fetch_balances: ACCOUNTS.map((a, i) => ({
    userid: a.userid,
    available: 13769 - i * 1100,
    daily: 4094,
    balance: 9675,
    error: "",
  })),
  fetch_tasks: {
    tasks: [
      { key: "first_message", label: "首次对话", points: 500, done: true },
      { key: "pick_skill", label: "选择技能", points: 1000, done: true },
      { key: "generate_ppt", label: "生成 PPT", points: 1500, done: true },
      { key: "set_schedule", label: "设置日程", points: 1000, done: true },
      { key: "install_skill", label: "安装技能", points: 1500, done: true },
      { key: "configure_remote", label: "配置远程", points: 1000, done: true },
      { key: "create_soul", label: "创建人格", points: 1500, done: true },
      { key: "share_soul", label: "分享人格", points: 2000, done: true },
    ],
    earned: 10000,
    total: 10000,
  },
  fetch_activation: { activated: true, appliedCode: "ABC123" },
  fetch_my_invites: [
    { code: "AA11BB", usedCount: 0, maxUses: 1, status: "active" },
    { code: "CC22DD", usedCount: 0, maxUses: 1, status: "active" },
    { code: "EE33FF", usedCount: 1, maxUses: 1, status: "exhausted" },
  ],
  // Tauri 的 getVersion() 走这个命令（App.tsx 用它显示版本号）
  "plugin:app|version": "2.0.0-measure",
  system_status: {
    found: true,
    roots: ["C:\\Users\\Public\\Loomy\\000000000000"],
    root: "C:\\Users\\Public\\Loomy\\000000000000",
    authFile: "C:\\Users\\Public\\Loomy\\000000000000\\userData\\auth-session.json",
    leveldbDirs: ["C:\\Users\\AppData\\Roaming\\Loomy\\Local Storage\\leveldb"],
    leveldbLog: "C:\\Users\\AppData\\Roaming\\Loomy\\Local Storage\\leveldb\\000003.log",
    exe: "C:\\Program Files\\Loomy\\Loomy.exe",
    runningPids: [],
    dataDir: "C:\\Users\\AppData\\Roaming\\LoomyAccountManager",
    accountsFile: "C:\\Users\\AppData\\Roaming\\LoomyAccountManager\\accounts.json",
    manualPath: null,
    guessCandidates: [],
    needsManual: false,
    reason: "",
  },
  set_loomy_path: "C:\\\\Users\\\\Public\\\\Loomy\\\\000000000000",
  clear_loomy_path: null,
  test_loomy_path: null,
  complete_all_tasks: [],
  bind_invite: null,
  claim_first_login: null,
  export_account: '{"session":"fake"}',
  delete_account: 6,
  update_account_note: null,
}

/** 打桩 Tauri IPC —— 必须在 App 被 import 之前执行。 */
function installTauriStub() {
  const w = window as unknown as Record<string, unknown>
  w.__TAURI_INTERNALS__ = {
    invoke: (cmd: string) => {
      if (cmd in MOCK) return Promise.resolve(MOCK[cmd])
      // 未打桩的命令返回 null 而不是抛错：布局测量不该被数据缺失打断
      console.warn("[measure] 未打桩的命令:", cmd)
      return Promise.resolve(null)
    },
    transformCallback: (cb: unknown) => cb,
    unregisterCallback: () => {},
    convertFileSrc: (p: string) => p,
    metadata: { currentWindow: { label: "main" }, currentWebview: { label: "main" } },
  }
}

/**
 * 测量台入口。
 *
 * # 设计决定：不做"点击导航再测量"
 *
 * 第一版是逐个点击侧栏项、再测量。这条路有两个坑，都踩过：
 *
 * 1. `click()` 只触发 `setState`，React 18 的更新是异步批处理的 ——
 *    紧接着测量会量到**上一页**的 DOM。表现是静默的：结果看起来正常，
 *    实际什么都没验证到（账号页的几何断言根本没执行）。
 * 2. 补上等待后，在无头浏览器的 `--virtual-time-budget` 下
 *    `requestAnimationFrame` 不保证触发，整段逻辑就卡住了。
 *
 * 所以改成：**用 `?page=` 在渲染前设好初始页**，一次只渲染并测量一页。
 * 没有点击 → 没有竞态 → 不需要等待。外部脚本按页面各跑一次即可。
 */

const PAGES = ["dashboard", "accounts", "import", "settings"] as const
type PageId = (typeof PAGES)[number]

const params = new URLSearchParams(location.search)
const wantPage = (params.get("page") ?? "accounts") as PageId
const page: PageId = (PAGES as readonly string[]).includes(wantPage) ? wantPage : "accounts"

// 在 import App 之前写死初始页（App 从 localStorage 读 activeMenu）
try {
  localStorage.setItem("activeMenu", page)
  // 侧栏折叠状态固定为"展开"，让测量结果可比（否则折叠与否会改变网格宽度）
  localStorage.setItem("sidebar-collapsed", "false")
  localStorage.setItem("theme", "dark")
} catch {
  /* localStorage 不可用就按默认 */
}

installTauriStub()
render()

/** 量当前页面，返回可读文本。 */
function measure(label: string): string {
  const d = document.documentElement
  const lines: string[] = []
  lines.push(`── ${label} ──`)
  lines.push(
    `视口 ${d.clientWidth}x${d.clientHeight} · 页面 scrollWidth ${d.scrollWidth}（溢出 ${d.scrollWidth - d.clientWidth}px）`,
  )

  const bad: Array<{ tag: string; cls: string; over: number; sw: number; cw: number; text: string }> = []
  for (const el of Array.from(document.querySelectorAll<HTMLElement>("*"))) {
    if (el.clientWidth === 0) continue
    if (el.id === "measure-result") continue
    if (el.classList.contains("pulse")) continue
    const st = getComputedStyle(el)
    if (st.overflowX === "auto" || st.overflowX === "scroll") continue
    if (el.scrollWidth > el.clientWidth + 1) {
      bad.push({
        tag: el.tagName,
        cls: String(el.className).slice(0, 44),
        over: el.scrollWidth - el.clientWidth,
        sw: el.scrollWidth,
        cw: el.clientWidth,
        text: (el.textContent ?? "").trim().replace(/\s+/g, " ").slice(0, 44),
      })
    }
  }
  bad.sort((a, b) => b.over - a.over)

  if (bad.length === 0) lines.push("  ✓ 无横向溢出")
  else {
    lines.push(`  ✗ ${bad.length} 个横向溢出（降序）：`)
    for (const b of bad.slice(0, 10)) {
      lines.push(`    +${b.over}px  ${b.tag}.${b.cls}  (${b.sw} vs ${b.cw})`)
      if (b.text) lines.push(`            「${b.text}」`)
    }
  }

  return lines.join("\n")
}

/**
 * 账号页的**几何断言**：详情栏必须在右边、与网格同行、不被裁。
 *
 * 这是用户明确提出的要求（"固定在右边，不要被挤到下面去"），
 * 所以要有确定的判据，而不是靠看截图。
 */
function checkAside(): string {
  const split = document.querySelector<HTMLElement>(".acct-split")
  const aside = document.querySelector<HTMLElement>(".acct-detail-aside")
  const grid = document.querySelector<HTMLElement>(".acct-grid")
  if (!split || !aside || !grid) {
    const missing = [!split && ".acct-split", !aside && ".acct-detail-aside", !grid && ".acct-grid"]
      .filter(Boolean)
      .join(" ")
    return `  ✗ 找不到布局元素：${missing}（这一页没测到）`
  }

  const a = aside.getBoundingClientRect()
  const g = grid.getBoundingClientRect()
  const s = split.getBoundingClientRect()
  const out: string[] = []
  out.push(
    `  布局 split=${Math.round(s.width)} grid=${Math.round(g.width)} aside=${Math.round(a.width)}`,
  )

  // ① 在右边
  if (a.left >= g.right - 2) {
    out.push(`  ✓ 详情栏在右侧（aside.left=${Math.round(a.left)} ≥ grid.right=${Math.round(g.right)}）`)
  } else {
    out.push(
      `  ✗ 详情栏不在右侧！aside.left=${Math.round(a.left)} < grid.right=${Math.round(g.right)} → 被挤到下面了`,
    )
  }

  // ② 同一行
  const dTop = Math.abs(a.top - g.top)
  out.push(
    dTop < 30
      ? `  ✓ 与网格同行（top 差 ${Math.round(dTop)}px）`
      : `  ✗ 不在同一行（top 差 ${Math.round(dTop)}px）→ 掉到下面了`,
  )

  // ③ 右边缘不被裁
  out.push(
    a.right <= s.right + 1
      ? `  ✓ 右边缘未裁（aside.right=${Math.round(a.right)} ≤ split.right=${Math.round(s.right)}）`
      : `  ✗ 右边缘被裁 ${Math.round(a.right - s.right)}px`,
  )

  // ④ 卡片网格里至少有一张卡片（否则断言在空页面上"通过"没有意义）
  const cards = document.querySelectorAll(".acct-card").length
  out.push(cards > 0 ? `  ✓ 网格里有 ${cards} 张卡片` : "  ✗ 网格里没有卡片（测试数据没加载）")

  return out.join("\n")
}

setTimeout(() => {
  const lines = [measure(`第 ${page} 页`)]
  // 只有账号页有右侧详情栏
  if (page === "accounts") {
    lines.push(checkAside())
  }
  const text = lines.join("\n")

  const d = document.documentElement
  const over = d.scrollWidth > d.clientWidth + 1
  const asideOk =
    page !== "accounts" || (text.includes("详情栏在右侧") && text.includes("与网格同行"))

  if (!params.has("nooverlay")) {
    const pre = document.createElement("pre")
    pre.id = "measure-result"
    pre.textContent = text
    pre.style.cssText =
      "position:fixed;left:0;top:0;z-index:99999;background:#000;color:#eee;padding:14px;margin:0;font:12px/1.5 monospace;white-space:pre-wrap;max-width:100%;overflow:auto;max-height:100%"
    document.body.appendChild(pre)
  }

  // 用 title 让外部脚本一眼判断成败（含具体原因）
  document.title = over
    ? `OVERFLOW:${d.scrollWidth - d.clientWidth}`
    : !asideOk
      ? "ASIDE_NOT_FIXED"
      : params.has("nooverlay")
        ? "READY"
        : "OK"
}, 1200)
