#!/usr/bin/env node
/**
 * Loomy 账号管理器核心（CLI manager.mjs 与 Web server.mjs 共用）
 *
 * 原理（见协议报告 §4.5）：
 *   本地 Loomy 登录态 = userData/auth-session.json（electron-store：{session,userid,phone,updatedAt}）
 *   + opencode/opencode.json 的 provider.imodel.options.apiKey（useSessionAuth=true，apiKey 即 session）
 *
 * 切换 = 备份当前两处 → 写入新账号 session → （可选）重启 Loomy 应用
 */

import fs from 'node:fs'
import path from 'node:path'
import os from 'node:os'
import { spawn, spawnSync } from 'node:child_process'
import { fileURLToPath } from 'node:url'

const SCRIPT_DIR = path.dirname(fileURLToPath(import.meta.url))
export const STORE_FILE = path.join(SCRIPT_DIR, 'managed-accounts.json')
export const BACKUP_DIR = path.join(SCRIPT_DIR, 'backups')
export const EXPORT_DIR = path.join(SCRIPT_DIR, 'tokens')

// ── Loomy 安装发现 ──────────────────────────────────────────────────────────
export function findLoomyRoots() {
  const roots = []
  const publicBase = path.join('C:\\Users\\Public', 'Loomy')
  try {
    for (const entry of fs.readdirSync(publicBase)) {
      const root = path.join(publicBase, entry)
      const authFile = path.join(root, 'userData', 'auth-session.json')
      if (fs.existsSync(authFile)) roots.push({ root, authFile })
    }
  } catch { /* 目录不存在则跳过 */ }
  const altRoot = path.join(os.homedir(), 'AppData', 'Roaming', 'loomy')
  const alt = path.join(altRoot, 'auth-session.json')
  if (fs.existsSync(alt)) roots.push({ root: altRoot, authFile: alt })
  return roots
}

/** 找出 Loomy 根下所有 opencode.json（跳过 skills/templates/cache） */
export function findOpencodeFiles(root) {
  const out = []
  const skip = new Set(['skills', 'templates', 'cache', 'node_modules'])
  const walk = (dir) => {
    let entries = []
    try { entries = fs.readdirSync(dir, { withFileTypes: true }) } catch { return }
    for (const e of entries) {
      if (skip.has(e.name)) continue
      const full = path.join(dir, e.name)
      if (e.isDirectory()) walk(full)
      else if (e.name === 'opencode.json') out.push(full)
    }
  }
  walk(root)
  return out
}

// ── 托管账号存储（managed-accounts.json）───────────────────────────────────
export function readStore() {
  try {
    const l = JSON.parse(fs.readFileSync(STORE_FILE, 'utf-8'))
    return Array.isArray(l) ? l : []
  } catch { return [] }
}
export function writeStore(list) {
  fs.writeFileSync(STORE_FILE, JSON.stringify(list, null, 2), 'utf-8')
}
export function upsertAccount(acct) {
  const list = readStore()
  const i = list.findIndex((a) => a.userid === acct.userid)
  if (i >= 0) list[i] = { ...list[i], ...acct }
  else list.push(acct)
  writeStore(list)
  return { created: i < 0 }
}
export function findAccount(arg) {
  const list = readStore()
  if (!list.length) return null
  const a = String(arg)
  return list.find((x) => x.userid === a)
    || list.find((x) => x.phone === a)
    || list[Number(a) - 1] || null
}

// ── 当前激活账号 ────────────────────────────────────────────────────────────
export function readActiveSession(root) {
  try {
    const j = JSON.parse(fs.readFileSync(root.authFile, 'utf-8'))
    if (!j?.session) return null
    return { session: j.session, userid: j.userid || '', phone: j.phone || '' }
  } catch { return null }
}

// ── 备份 / 恢复 ─────────────────────────────────────────────────────────────
export function backupNow(root) {
  const stamp = new Date().toISOString().replace(/[:.]/g, '-').slice(0, 19)
  const dir = path.join(BACKUP_DIR, stamp)
  fs.mkdirSync(dir, { recursive: true })
  fs.copyFileSync(root.authFile, path.join(dir, 'auth-session.json'))
  for (const f of findOpencodeFiles(root.root)) {
    const dest = path.join(dir, path.relative(root.root, f))
    fs.mkdirSync(path.dirname(dest), { recursive: true })
    fs.copyFileSync(f, dest)
  }
  fs.writeFileSync(path.join(dir, 'meta.json'), JSON.stringify({ root: root.root, at: Date.now() }, null, 2))
  return dir
}

export function listBackups() {
  try {
    return fs.readdirSync(BACKUP_DIR)
      .filter((d) => fs.existsSync(path.join(BACKUP_DIR, d, 'auth-session.json')))
      .sort().reverse()
  } catch { return [] }
}

export function restoreBackup(root, backupName) {
  const dir = path.join(BACKUP_DIR, backupName)
  if (!fs.existsSync(path.join(dir, 'auth-session.json'))) throw new Error(`备份不存在: ${backupName}`)
  fs.copyFileSync(path.join(dir, 'auth-session.json'), root.authFile)
  const walk = (d) => {
    let entries = []
    try { entries = fs.readdirSync(d, { withFileTypes: true }) } catch { return }
    for (const e of entries) {
      const full = path.join(d, e.name)
      if (e.isDirectory()) walk(full)
      else if (e.name === 'opencode.json') {
        fs.copyFileSync(full, path.join(root.root, path.relative(dir, full)))
      }
    }
  }
  walk(dir)
  return dir
}

// ── 切换（应用 session 到本地 Loomy）───────────────────────────────────────
export function applySession(root, acct) {
  const payload = {
    session: acct.session,
    userid: acct.userid,
    phone: acct.phone || '',
    updatedAt: Date.now(),
  }
  fs.writeFileSync(root.authFile, JSON.stringify(payload, null, 2), 'utf-8')
  for (const f of findOpencodeFiles(root.root)) {
    try {
      const cfg = JSON.parse(fs.readFileSync(f, 'utf-8'))
      if (cfg?.provider?.imodel?.options) {
        cfg.provider.imodel.options.apiKey = acct.session
        fs.writeFileSync(f, JSON.stringify(cfg, null, 2), 'utf-8')
        console.log(`  更新 ${path.relative(root.root, f)} → imodel.apiKey=session`)
      }
    } catch { /* 非 JSON 或结构不符则跳过 */ }
  }
  return payload
}

// ── 重启 Loomy 应用（可选，无感切换用）────────────────────────────────────
export function restartLoomy(exePath) {
  spawnSync('taskkill', ['/IM', 'Loomy.exe', '/F'], { stdio: 'ignore' })
  if (exePath && fs.existsSync(exePath)) {
    const p = spawn(exePath, [], { detached: true, stdio: 'ignore' })
    p.unref()
    return { killed: true, relaunched: exePath }
  }
  return { killed: true, relaunched: null, note: '未找到 Loomy.exe，请手动启动应用' }
}

export function isLoomyRunning() {
  try {
    const r = spawnSync('tasklist', ['/FI', 'IMAGENAME eq Loomy.exe', '/NH'], { encoding: 'utf8' })
    return /Loomy\.exe/i.test(r.stdout || '')
  } catch { return false }
}

// ── 导入 / 导出 ─────────────────────────────────────────────────────────────
/** 解析 token 文件：支持 {session,userid,phone,...} / 裸 session 串 */
export function parseTokenFile(filePath) {
  const raw = fs.readFileSync(filePath, 'utf-8').trim()
  try {
    const j = JSON.parse(raw)
    if (typeof j === 'string') return { session: j }
    return j
  } catch {
    if (/^[0-9a-f]{16,64}$/i.test(raw)) return { session: raw }
  }
  return null
}

export function buildExportName(acct) {
  return `loomy-${acct.userid}.json`
}
