import fs from 'node:fs'
import path from 'node:path'
import { createRequire } from 'node:module'

const require = createRequire(import.meta.url)

function listFiles(dir) {
  try {
    return fs.readdirSync(dir, { withFileTypes: true })
  } catch {
    return []
  }
}

function ensureDir(dir) {
  fs.mkdirSync(dir, { recursive: true })
}

function copyFile(src, dst) {
  fs.copyFileSync(src, dst)
}

function main() {
  const projectRoot = process.cwd()
  const distAssetsDir = path.join(projectRoot, 'dist', 'assets')
  ensureDir(distAssetsDir)

  const avplayerEntry = require.resolve('@libmedia/avplayer')
  let avplayerPkgDir = path.dirname(avplayerEntry)
  // Walk upwards until we find the package root (package.json).
  for (let i = 0; i < 8; i++) {
    const p = path.join(avplayerPkgDir, 'package.json')
    if (fs.existsSync(p)) break
    const next = path.dirname(avplayerPkgDir)
    if (next === avplayerPkgDir) break
    avplayerPkgDir = next
  }

  const avplayerEsmDir = path.join(avplayerPkgDir, 'dist', 'esm')

  const entries = listFiles(avplayerEsmDir)
  const chunks = entries
    .filter((e) => e.isFile())
    .map((e) => e.name)
    .filter((name) => /^\d+\.avplayer\.js$/.test(name))

  if (chunks.length === 0) {
    console.warn('[copy-libmedia-chunks] no *.avplayer.js chunks found in', avplayerEsmDir)
    return
  }

  for (const name of chunks) {
    const src = path.join(avplayerEsmDir, name)
    const dst = path.join(distAssetsDir, name)
    copyFile(src, dst)
  }

  console.log(`[copy-libmedia-chunks] copied ${chunks.length} chunks to ${distAssetsDir}`)
}

main()
