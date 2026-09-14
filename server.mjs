#!/usr/bin/env node
/**
 * Loomy 账号管理器 Web 前端（零依赖，Node ≥18，仅监听 127.0.0.1）
 *
 * 启动：node server.mjs            →  http://127.0.0.1:3091
 * 端口：环境变量 LOOMY_WEB_PORT 覆盖（默认 3091）
 *
 * API：
 *   GET  /              页面
 *   GET  /api/status    本机 Loomy 根 + 当前激活账号 + 托管账号 + 备份 + 运行状态
 *   POST /api/scan      扫描注册机目录纳入托管 {dirs?: [..]}
 *   POST /api/import    导入 {session, userid, phone?, nickname?, name?, doSwitch?}
 *   POST /api/switch    切换 {userid, restart?}
 *   POST /api/export    导出 token {userid} → 写 tokens/ 并返回内容
 *   POST /api/restore   恢复 {backup?}（默认最近一次）
 */

import http from 'node:http'
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import * as mgr from './manager-core.mjs'

const ROOT = path.dirname(fileURLToPath(import.meta.url))
const INDEX = path.join(ROOT, 'index.html')
const HOST = '127.0.0.1'
const PORT = Number(process.env.LOOMY_WEB_PORT || 3091)

function readBody(req) {
  return new Promise((resolve, reject) => {
    let data = ''
    req.on('data', (c) => { data += c; if (data.length > 1e6) req.destroy() })
    req.on('end', () => { try { resolve(data ? JSON.parse(data) : {}) } catch { reject(new Error('请求体不是合法 JSON')) } })
    req.on('error', reject)
  })
}

function send(res, code, obj, type = 'application/json; charset=utf-8') {
  res.writeHead(code, { 'Content-Type': type, 'Cache-Control': 'no-store' })
  res.end(typeof obj === 'string' ? obj : JSON.stringify(obj))
}

function status() {
  return {
    roots: mgr.findLoomyRoots().map((root) => ({
      root: root.root,
      authFile: root.authFile,
      active: mgr.readActiveSession(root),
    })),
    accounts: mgr.readStore(),
    backups: mgr.listBackups(),
    running: mgr.isLoomyRunning(),
    storeFile: mgr.STORE_FILE,
    exportDir: mgr.EXPORT_DIR,
  }
}

function scanAccounts(dirsArg) {
  const dirs = dirsArg && dirsArg.length
    ? dirsArg.map((d) => path.resolve(ROOT, d))
    : (() => {
      const d = [ROOT]
      const sibling = path.join(ROOT, '..', 'loomy-register-machine')
      if (fs.existsSync(sibling)) d.push(sibling)
      return d
    })()
  const scanned = []
  for (const dir of dirs) {
    try {
      const acc = JSON.parse(fs.readFileSync(path.join(dir, 'accounts.json'), 'utf-8'))
      for (const a of Array.isArray(acc) ? acc : []) {
        if (a?.userid && a?.session) scanned.push({ userid: a.userid, session: a.session, phone: a.phone || '', nickname: a.nickname || '', source: 'register' })
      }
    } catch { /* 无 accounts.json */ }
    try {
      const adir = path.join(dir, 'auths')
      for (const f of fs.readdirSync(adir)) {
        if (!f.endsWith('.json')) continue
        const j = JSON.parse(fs.readFileSync(path.join(adir, f), 'utf-8'))
        if (j?.userid && j?.session) scanned.push({ userid: j.userid, session: j.session, phone: j.phone || '', nickname: j.nickname || '', source: 'auths' })
      }
    } catch { /* 无 auths */ }
  }
  let added = 0
  for (const s of scanned) {
    const { created } = mgr.upsertAccount({ ...s, importedAt: Date.now() })
    if (created) added++
  }
  return { found: scanned.length, added }
}

function resolveExePath() {
  for (const base of [ROOT, path.join(ROOT, '..', 'loomy-register-machine')]) {
    try {
      const cfg = JSON.parse(fs.readFileSync(path.join(base, 'config.json'), 'utf-8'))
      if (cfg?.loomy?.resources) {
        const exe = path.join(path.dirname(cfg.loomy.resources), 'Loomy.exe')
        if (fs.existsSync(exe)) return exe
      }
    } catch { /* 无配置 */ }
  }
  const fallback = 'D:\\software\\Loomy-Setup-0.9.37\\Loomy.exe'
  return fs.existsSync(fallback) ? fallback : null
}

const server = http.createServer(async (req, res) => {
  const u = new URL(req.url, `http://${req.headers.host || 'localhost'}`)
  const p = u.pathname
  try {
    if (req.method === 'GET' && p === '/') {
      return send(res, 200, fs.readFileSync(INDEX, 'utf-8'), 'text/html; charset=utf-8')
    }
    if (req.method === 'GET' && p === '/api/status') return send(res, 200, status())
    if (req.method === 'POST' && p === '/api/scan') {
      const b = await readBody(req)
      return send(res, 200, { ok: true, ...scanAccounts(b.dirs) })
    }
    if (req.method === 'POST' && p === '/api/import') {
      const b = await readBody(req)
      if (!b.session || !b.userid) return send(res, 400, { error: '缺少 session 或 userid' })
      const { created } = mgr.upsertAccount({
        userid: String(b.userid),
        session: String(b.session),
        phone: String(b.phone || ''),
        nickname: String(b.nickname || ''),
        name: String(b.name || ''),
        source: 'web-import',
        importedAt: Date.now(),
      })
      if (b.doSwitch) {
        const root = mgr.findLoomyRoots()[0]
        if (!root) return send(res, 500, { error: '未找到本机 Loomy 安装' })
        const backupDir = mgr.backupNow(root)
        mgr.applySession(root, { userid: String(b.userid), session: String(b.session), phone: String(b.phone || '') })
        return send(res, 200, { ok: true, created, backupDir, switched: true })
      }
      return send(res, 200, { ok: true, created })
    }
    if (req.method === 'POST' && p === '/api/switch') {
      const b = await readBody(req)
      const acct = mgr.findAccount(b.userid)
      if (!acct) return send(res, 404, { error: '托管账号里找不到该账号' })
      const root = mgr.findLoomyRoots()[0]
      if (!root) return send(res, 500, { error: '未找到本机 Loomy 安装' })
      const backupDir = mgr.backupNow(root)
      const applied = mgr.applySession(root, acct)
      let restart = null
      if (b.restart) restart = mgr.restartLoomy(resolveExePath())
      return send(res, 200, { ok: true, backupDir, applied, restart, running: mgr.isLoomyRunning() })
    }
    if (req.method === 'POST' && p === '/api/export') {
      const b = await readBody(req)
      const acct = mgr.findAccount(b.userid)
      if (!acct) return send(res, 404, { error: '托管账号里找不到该账号' })
      fs.mkdirSync(mgr.EXPORT_DIR, { recursive: true })
      const file = path.join(mgr.EXPORT_DIR, mgr.buildExportName(acct))
      const payload = {
        session: acct.session, userid: acct.userid, phone: acct.phone || '',
        nickname: acct.nickname || '', name: acct.name || '', updatedAt: Date.now(),
      }
      fs.writeFileSync(file, JSON.stringify(payload, null, 2), 'utf-8')
      return send(res, 200, { ok: true, file, content: payload })
    }
    if (req.method === 'POST' && p === '/api/restore') {
      const b = await readBody(req)
      const root = mgr.findLoomyRoots()[0]
      if (!root) return send(res, 500, { error: '未找到本机 Loomy 安装' })
      const backups = mgr.listBackups()
      const name = b.backup || backups[0]
      if (!name || !backups.includes(name)) return send(res, 404, { error: '没有可用备份' })
      mgr.restoreBackup(root, name)
      return send(res, 200, { ok: true, backup: name, active: mgr.readActiveSession(root) })
    }
    send(res, 404, { error: 'not found' })
  } catch (e) {
    send(res, 500, { error: e.message })
  }
})

server.listen(PORT, HOST, () => {
  console.log(`Loomy 账号管理器 Web 前端已启动: http://${HOST}:${PORT}`)
  console.log(`工作目录: ${ROOT}`)
})
