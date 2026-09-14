#!/usr/bin/env node
/**
 * Loomy 账号管理器（CLI）
 *
 * 用法：
 *   node manager.mjs status                       本机 Loomy 当前账号 + 安装根
 *   node manager.mjs list                         托管账号列表（标注当前激活）
 *   node manager.mjs scan [扫描目录...]           把注册机(auths/accounts.json)的账号纳入托管
 *   node manager.mjs import <token文件> [--name 别名] [--switch]
 *   node manager.mjs import --session <s> --userid <u> [--phone <p>] [--nickname <n>] [--name 别名] [--switch]
 *   node manager.mjs switch <userid|手机号|序号> [--restart]
 *   node manager.mjs export <userid|手机号|序号> [--out 路径]
 *   node manager.mjs restore [备份名]             恢复之前的切换（默认最近一次）
 *   node manager.mjs backups                      列出备份
 */

import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import * as mgr from './manager-core.mjs'

const SCRIPT_DIR = path.dirname(fileURLToPath(import.meta.url))
const mask = (s, n = 8) => (s ? String(s).slice(0, n) + '…' : '-')
const t = () => new Date().toISOString().replace('T', ' ').slice(0, 19)
const log = (...a) => console.log(`[${t()}]`, ...a)

function help() {
  console.log(`Loomy 账号管理器
用法:
  node manager.mjs status                       本机 Loomy 当前账号 + 安装根
  node manager.mjs list                         托管账号列表（标注当前激活）
  node manager.mjs scan [扫描目录...]           把注册机账号纳入托管（默认扫本目录 + 兄弟目录 loomy-register-machine）
  node manager.mjs import <token文件> [--name 别名] [--switch]
  node manager.mjs import --session <s> --userid <u> [--phone <p>] [--nickname <n>] [--name 别名] [--switch]
  node manager.mjs switch <userid|手机号|序号> [--restart]
  node manager.mjs export <userid|手机号|序号> [--out 路径]
  node manager.mjs restore [备份名]
  node manager.mjs backups
选项:
  --switch    导入后立即切换为该账号
  --restart   切换后重启 Loomy 应用（无感生效）
  -h, --help  帮助`)
}

function parseFlags(args) {
  const flags = {}
  for (let i = 0; i < args.length; i++) {
    const a = args[i]
    if (a.startsWith('--')) {
      const key = a.slice(2)
      const val = args[i + 1]
      if (val !== undefined && !val.startsWith('--')) { flags[key] = val; i++ }
      else flags[key] = true
    }
  }
  return flags
}

function firstRoot() {
  const roots = mgr.findLoomyRoots()
  if (!roots.length) {
    console.error('未找到本机 Loomy 安装（C:\\Users\\Public\\Loomy\\*\\userData\\auth-session.json）')
    process.exit(1)
  }
  return roots[0]
}

async function cmdStatus() {
  for (const root of mgr.findLoomyRoots()) {
    const active = mgr.readActiveSession(root)
    const opencode = mgr.findOpencodeFiles(root.root)
    console.log(`Loomy 根: ${root.root}`)
    console.log(`  当前账号: ${active ? `userid=${active.userid} phone=${active.phone || '-'} session=${mask(active.session)}` : '（无/未登录）'}`)
    console.log(`  auth-session.json: ${root.authFile}`)
    console.log(`  opencode.json: ${opencode.join(', ') || '（无）'}`)
    console.log(`  运行中: ${mgr.isLoomyRunning() ? '是' : '否'}`)
  }
}

function cmdList() {
  const list = mgr.readStore()
  const root = mgr.findLoomyRoots()[0]
  const active = root ? mgr.readActiveSession(root) : null
  console.log(`托管账号 ${list.length} 个（存储: ${mgr.STORE_FILE}）`)
  console.log('  #  手机号        userid             session    昵称      来源      当前')
  list.forEach((a, i) => {
    const isActive = active && active.userid === a.userid
    console.log(`  ${String(i + 1).padEnd(3)} ${String(a.phone || '-').padEnd(13)} ${String(a.userid).padEnd(17)} ${mask(a.session).padEnd(11)} ${String(a.nickname || a.name || '-').padEnd(10)} ${String(a.source || '-').padEnd(9)} ${isActive ? '✔ 激活' : ''}`)
  })
  if (!list.length) console.log('（空。用 import / scan 添加账号）')
}

function scanDirs(args) {
  const explicit = args.filter((a) => !a.startsWith('--'))
  if (explicit.length) return explicit.map((d) => path.resolve(SCRIPT_DIR, d))
  const dirs = [SCRIPT_DIR]
  const sibling = path.join(SCRIPT_DIR, '..', 'loomy-register-machine')
  if (fs.existsSync(sibling)) dirs.push(sibling)
  return dirs
}

function cmdScan(args) {
  const scanned = []
  for (const dir of scanDirs(args)) {
    try {
      const acc = JSON.parse(fs.readFileSync(path.join(dir, 'accounts.json'), 'utf-8'))
      for (const a of Array.isArray(acc) ? acc : []) {
        if (a?.userid && a?.session) scanned.push({ userid: a.userid, session: a.session, phone: a.phone || '', nickname: a.nickname || '', source: 'register' })
      }
    } catch { /* 该目录无 accounts.json */ }
    try {
      const adir = path.join(dir, 'auths')
      for (const f of fs.readdirSync(adir)) {
        if (!f.endsWith('.json')) continue
        const j = JSON.parse(fs.readFileSync(path.join(adir, f), 'utf-8'))
        if (j?.userid && j?.session) scanned.push({ userid: j.userid, session: j.session, phone: j.phone || '', nickname: j.nickname || '', source: 'auths' })
      }
    } catch { /* 该目录无 auths */ }
  }
  let added = 0
  for (const s of scanned) {
    const { created } = mgr.upsertAccount({ ...s, importedAt: Date.now() })
    if (created) { added++; console.log(`  + ${s.userid} ${s.phone || ''}`) }
  }
  console.log(`扫描完成：发现 ${scanned.length} 个，新增 ${added} 个（按 userid 去重）`)
}

async function cmdImport(args) {
  const flags = parseFlags(args)
  let acct = null
  const fileArg = args.find((a) => !a.startsWith('--'))
  if (fileArg) {
    if (!fs.existsSync(fileArg)) { console.error(`文件不存在: ${fileArg}`); process.exit(1) }
    acct = mgr.parseTokenFile(fileArg)
    if (!acct) { console.error('无法解析 token 文件（需要 JSON 或裸 session 串）'); process.exit(1) }
  } else if (flags.session) {
    acct = { session: flags.session, userid: flags.userid || '', phone: flags.phone || '', nickname: flags.nickname || '' }
  }
  if (!acct?.session) { console.error('缺少 session（用文件或 --session）'); process.exit(1) }
  if (!acct.userid) {
    console.error('缺少 userid（token 文件里没有的话用 --userid 指定）')
    process.exit(1)
  }
  const entry = {
    userid: String(acct.userid),
    session: String(acct.session),
    phone: String(acct.phone || ''),
    nickname: String(acct.nickname || ''),
    name: flags.name || '',
    source: fileArg ? `import:${path.basename(fileArg)}` : 'import',
    importedAt: Date.now(),
  }
  const { created } = mgr.upsertAccount(entry)
  console.log(`${created ? '导入' : '更新'}账号 userid=${entry.userid} phone=${entry.phone || '-'} session=${mask(entry.session)}`)
  if (flags.switch) await doSwitch(entry.userid, flags)
}

function resolveExePath() {
  // 本目录 config.json（可选）→ 兄弟注册机目录 config.json → 默认安装路径
  for (const base of [SCRIPT_DIR, path.join(SCRIPT_DIR, '..', 'loomy-register-machine')]) {
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

async function doSwitch(arg, flags) {
  const acct = mgr.findAccount(arg)
  if (!acct) { console.error(`托管账号里找不到: ${arg}（先 import/scan）`); process.exit(1) }
  const root = firstRoot()
  const backupDir = mgr.backupNow(root)
  console.log(`备份当前登录态 → ${backupDir}`)
  const before = mgr.readActiveSession(root)
  const payload = mgr.applySession(root, acct)
  console.log(`切换完成 → userid=${payload.userid} phone=${payload.phone || '-'} session=${mask(payload.session)}`)
  if (before) console.log(`（原账号 userid=${before.userid} 可通过 restore 恢复）`)
  if (flags.restart) {
    const exe = resolveExePath()
    const r = mgr.restartLoomy(exe)
    console.log(`已重启 Loomy: ${r.relaunched || r.note || 'done'}`)
  } else if (mgr.isLoomyRunning()) {
    console.log('提示: Loomy 正在运行，重启后新账号才会生效（加 --restart 自动重启）')
  }
}

function cmdExport(arg, flags) {
  const acct = mgr.findAccount(arg)
  if (!acct) { console.error(`托管账号里找不到: ${arg}`); process.exit(1) }
  const out = flags.out || path.join(mgr.EXPORT_DIR, mgr.buildExportName(acct))
  fs.mkdirSync(path.dirname(out), { recursive: true })
  const payload = { session: acct.session, userid: acct.userid, phone: acct.phone || '', nickname: acct.nickname || '', name: acct.name || '', updatedAt: Date.now() }
  fs.writeFileSync(out, JSON.stringify(payload, null, 2), 'utf-8')
  console.log(`已导出 → ${out}`)
  console.log('对方拿到后: node manager.mjs import <该文件> --switch')
}

function cmdRestore(arg) {
  const root = firstRoot()
  const backups = mgr.listBackups()
  if (!backups.length) { console.error('没有可用备份'); process.exit(1) }
  const name = arg || backups[0]
  if (!backups.includes(name)) { console.error(`备份不存在: ${name}，可用: ${backups.join(', ')}`); process.exit(1) }
  const dir = mgr.restoreBackup(root, name)
  const active = mgr.readActiveSession(root)
  console.log(`已恢复 ${dir}`)
  console.log(`当前账号: userid=${active?.userid} session=${mask(active?.session)}`)
}

function cmdBackups() {
  const backups = mgr.listBackups()
  console.log(`备份 ${backups.length} 个（${mgr.BACKUP_DIR}）`)
  backups.forEach((b, i) => console.log(`  ${i === 0 ? '*' : ' '} ${b}`))
}

async function main() {
  const [cmd, ...args] = process.argv.slice(2)
  const flags = parseFlags(args)
  if (flags.help || !cmd) { help(); return }
  switch (cmd) {
    case 'status': await cmdStatus(); break
    case 'list': cmdList(); break
    case 'scan': cmdScan(args); break
    case 'import': await cmdImport(args); break
    case 'switch': await doSwitch(args.find((a) => !a.startsWith('--')), flags); break
    case 'export': cmdExport(args.find((a) => !a.startsWith('--')), flags); break
    case 'restore': cmdRestore(args.find((a) => !a.startsWith('--'))); break
    case 'backups': cmdBackups(); break
    default: console.error(`未知命令: ${cmd}\n`); help(); process.exit(1)
  }
}

main().catch((e) => { console.error('ERR', e); process.exit(1) })
