/**
 * 主题注册表 —— 全站唯一事实来源。
 *
 * # 为什么要有"唯一来源"（抄 Kiro 的教训）
 *
 * Kiro 那个项目踩过这个坑：主题清单散在两处（侧栏轮转表 + 设置页列表），
 * 两份**互相矛盾** —— 一处有两个"幽灵主题"（CSS 里根本没有对应规则，
 * 切过去 `<html data-theme>` 匹配不到任何东西，主题直接丢失），
 * 另一处又缺几个真实存在的主题（切到它时 `indexOf` 得 -1 → 意外跳回第一个）。
 *
 * 所以这里：
 *
 * - 本文件是**唯一**来源，下面每个 key 都在 `index.css` 里有 `[data-theme='<key>']`
 * - 侧栏轮转与设置页列表都从 `THEME_LIST` 派生
 * - `findTheme` 找不到时返回 undefined，调用方必须兜底（避免幽灵主题）
 */

export type ThemeIconName = "Monitor" | "Sun" | "Moon" | "Palette"

export interface ThemeOption {
  /** 与 index.css 的 `[data-theme='<key>']` 一一对应 */
  key: string
  /** 展示名 */
  name: string
  iconName: ThemeIconName
  /** 预览用的渐变色 */
  color: string
  /** 是否深色系（决定一些细节，例如阴影强度） */
  dark: boolean
}

export const THEME_LIST: ThemeOption[] = [
  { key: "system", name: "跟随系统", iconName: "Monitor", color: "linear-gradient(135deg,#94a3b8,#475569)", dark: false },
  { key: "light", name: "浅色", iconName: "Sun", color: "linear-gradient(135deg,#60a5fa,#2563eb)", dark: false },
  { key: "dark", name: "深色", iconName: "Moon", color: "linear-gradient(135deg,#374151,#030712)", dark: true },
  { key: "tech", name: "科技蓝", iconName: "Palette", color: "linear-gradient(135deg,#22d3ee,#0284c7)", dark: true },
  { key: "midnight", name: "午夜", iconName: "Moon", color: "linear-gradient(135deg,#fbbf24,#000000)", dark: true },
  { key: "purple", name: "紫色", iconName: "Palette", color: "linear-gradient(135deg,#a78bfa,#6d28d9)", dark: false },
  { key: "ocean", name: "海洋", iconName: "Palette", color: "linear-gradient(135deg,#38bdf8,#0369a1)", dark: false },
  { key: "forest", name: "森林", iconName: "Palette", color: "linear-gradient(135deg,#34d399,#065f46)", dark: false },
  { key: "sunset", name: "日落", iconName: "Palette", color: "linear-gradient(135deg,#fb923c,#dc2626)", dark: false },
  { key: "rose", name: "玫瑰", iconName: "Palette", color: "linear-gradient(135deg,#fb7185,#be123c)", dark: false },
  { key: "business", name: "商务", iconName: "Palette", color: "linear-gradient(135deg,#fbbf24,#b45309)", dark: false },
  { key: "sakura", name: "樱花", iconName: "Palette", color: "linear-gradient(135deg,#fbcfe8,#f43f5e)", dark: false },
]

/** 主题 key 顺序（侧栏轮转用）。 */
export const THEME_KEYS: string[] = THEME_LIST.map((t) => t.key)

/** 查主题。找不到返回 undefined —— 调用方必须兜底，别切到幽灵主题。 */
export const findTheme = (key: string | undefined): ThemeOption | undefined =>
  THEME_LIST.find((t) => t.key === key)

/** 应用到 `<html data-theme>`；system 时按系统偏好解析成 light/dark。 */
export function applyTheme(key: string): void {
  const resolved =
    key === "system"
      ? window.matchMedia("(prefers-color-scheme: dark)").matches
        ? "dark"
        : "light"
      : key
  const el = document.documentElement
  // `system` 也写进去：CSS 用它做兜底，而实际配色由 resolved 决定
  el.setAttribute("data-theme", resolved)
  el.setAttribute("data-theme-pref", key)
  el.style.colorScheme = findTheme(resolved)?.dark ? "dark" : "light"
}

/** 侧栏轮转的下一个主题。 */
export function nextTheme(cur: string): string {
  const i = THEME_KEYS.indexOf(cur)
  // cur 不在表里（历史遗留值）时从头开始，避免 (-1+1)%n = 0 之外的意外
  return THEME_KEYS[i < 0 ? 0 : (i + 1) % THEME_KEYS.length]
}
