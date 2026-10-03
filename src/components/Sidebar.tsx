/**
 * 侧栏导航 —— 抄 Kiro 的 Layout（hj01857655/kiro-account-manager）。
 *
 * # 抄了什么
 *
 * | Kiro 的设计 | 这里的实现 |
 * |---|---|
 * | 可折叠侧栏（192px ↔ 64px） | `transition: width`，折叠状态存 localStorage |
 * | 品牌区（点一下折叠） | `.sidebar-brand`，hover 时 logo 微放大 |
 * | 导航项：图标 + 文字 + 激活态左侧竖条 | `.nav-item` + `.active::before` |
 * | 底部：主题轮转按钮 + 折叠按钮 + 版本号 | `.sidebar-foot` |
 * | 玻璃拟态背景 | `.glass-sidebar`（backdrop-filter） |
 *
 * # 为什么不引它的 Tailwind + shadcn/ui
 *
 * 它那套的价值在"组件数量多"（它有 8 个页面、几十个组件）。
 * 本应用只有 4 个页面、十几个组件 —— 引一整套框架的构建复杂度
 * 大于收益。但**设计语言与信息层级照抄**（那才是"布局"）。
 */

import {
  Boxes,
  ChevronLeft,
  ChevronRight,
  KeyRound,
  Monitor,
  Moon,
  Palette,
  Sun,
  Upload,
  type LucideIcon,
} from "lucide-react"
import type { ThemeIconName } from "../lib/theme"
import { findTheme, nextTheme } from "../lib/theme"

export type PageId = "dashboard" | "accounts" | "import" | "settings"

const THEME_ICONS: Record<ThemeIconName, LucideIcon> = {
  Monitor,
  Sun,
  Moon,
  Palette,
}

export interface NavEntry {
  id: PageId
  label: string
  icon: LucideIcon
  /** 右侧小角标（例如账号数量） */
  count?: number
  desc?: string
}

export default function Sidebar({
  active,
  onNavigate,
  collapsed,
  onToggleCollapse,
  theme,
  onThemeChange,
  version,
  connection,
}: {
  active: PageId
  onNavigate: (id: PageId) => void
  collapsed: boolean
  onToggleCollapse: () => void
  theme: string
  onThemeChange: (t: string) => void
  version: string
  connection: { ok: boolean; text: string }
}) {
  const nav: NavEntry[] = [
    { id: "dashboard", label: "仪表盘", icon: Boxes },
    { id: "accounts", label: "账号管理", icon: KeyRound },
    { id: "import", label: "导入账号", icon: Upload },
    { id: "settings", label: "设置", icon: Palette },
  ]

  const cur = findTheme(theme)
  const ThemeIcon = (cur && THEME_ICONS[cur.iconName]) || Sun

  return (
    <aside className="sidebar glass-sidebar" style={{ width: collapsed ? 64 : 196 }}>
      {/* 品牌区：点一下折叠（抄 Kiro 的交互） */}
      <div
        className="sidebar-brand"
        onClick={onToggleCollapse}
        title={collapsed ? "展开侧栏" : "收起侧栏"}
        style={{ padding: collapsed ? "14px 0 10px" : undefined, justifyContent: collapsed ? "center" : undefined }}
      >
        <div className="sidebar-logo">L</div>
        {!collapsed && (
          <div className="sidebar-title">
            <span className="t1">LOOMY</span>
            <span className="t2">Account Manager</span>
          </div>
        )}
      </div>

      {/* 导航 */}
      <nav className="sidebar-nav">
        {nav.map((item) => {
          const Icon = item.icon
          const isActive = active === item.id
          return (
            <button
              key={item.id}
              className={`nav-item ${isActive ? "active" : ""}`}
              onClick={() => onNavigate(item.id)}
              title={collapsed ? item.label : undefined}
              style={{ justifyContent: collapsed ? "center" : undefined }}
            >
              <Icon size={17} style={{ flexShrink: 0 }} />
              {!collapsed && (
                <>
                  <span className="label">{item.label}</span>
                  {item.count != null && item.count > 0 && (
                    <span className="count">{item.count}</span>
                  )}
                </>
              )}
            </button>
          )
        })}
      </nav>

      {/* 底部：连接状态 + 主题 + 折叠 + 版本 */}
      <div className="sidebar-foot" style={{ flexDirection: "column", alignItems: "stretch", gap: 8 }}>
        {!collapsed ? (
          <div
            className="row"
            style={{ gap: 7, fontSize: 11.5, color: "var(--muted-fg)", padding: "0 2px" }}
          >
            {connection.ok ? (
              <span className="pulse" />
            ) : (
              <span
                style={{
                  width: 7,
                  height: 7,
                  borderRadius: "50%",
                  background: "var(--err)",
                  flexShrink: 0,
                }}
              />
            )}
            <span style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
              {connection.text}
            </span>
          </div>
        ) : (
          <div style={{ display: "flex", justifyContent: "center" }}>
            {connection.ok ? <span className="pulse" /> : <span style={{ width: 7, height: 7, borderRadius: "50%", background: "var(--err)" }} />}
          </div>
        )}

        <div className="row between" style={{ gap: 4 }}>
          <button
            className="icon-btn"
            onClick={() => onThemeChange(nextTheme(theme))}
            title={`主题：${cur?.name ?? theme}（点击切换）`}
          >
            <ThemeIcon size={15} />
          </button>

          {!collapsed && <span className="sidebar-ver">v{version}</span>}

          <button
            className="icon-btn"
            onClick={onToggleCollapse}
            title={collapsed ? "展开" : "收起"}
          >
            {collapsed ? <ChevronRight size={15} /> : <ChevronLeft size={15} />}
          </button>
        </div>
      </div>
    </aside>
  )
}
