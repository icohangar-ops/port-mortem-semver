'use strict'

// Copies tests/original/ into adapter/test/ before every `npm test`.
//
// A symlink would be tidier, but Node resolves a module's realpath before
// resolving its requires, so every test file symlinked from tests/original/
// would look for `../../functions/compare.js` next to the originals instead of
// next to the adapter. `--preserve-symlinks-main` fixes that for the test file
// itself but not for the sub-processes some tests spawn, so the tests are
// copied verbatim instead. They are never edited: this script mirrors the
// source of truth and deletes anything that has drifted.

const fs = require('fs')
const path = require('path')

const SRC = path.resolve(__dirname, '..', '..', 'tests', 'original')
const DEST = path.resolve(__dirname, '..', 'test')

// Resolve `name` under `parent` and reject anything that escapes `root`
// (relative `..`, absolute segments, or a symlink whose realpath leaves root).
const confine = (root, parent, name) => {
  if (!name || path.basename(name) !== name || name === '.' || name === '..') {
    throw new Error(`refusing path traversal: ${name}`)
  }
  const resolved = path.resolve(parent, name)
  const rel = path.relative(root, resolved)
  if (rel.startsWith(`..${path.sep}`) || rel === '..' || path.isAbsolute(rel)) {
    throw new Error(`path escapes ${root}: ${name}`)
  }
  try {
    const real = fs.realpathSync(resolved)
    const realRoot = fs.realpathSync(root)
    const realRel = path.relative(realRoot, real)
    if (realRel.startsWith(`..${path.sep}`) || realRel === '..' || path.isAbsolute(realRel)) {
      throw new Error(`path escapes ${root}: ${name}`)
    }
  } catch (err) {
    if (err.code !== 'ENOENT') {
      throw err
    }
  }
  return resolved
}

const sync = (src, dest) => {
  fs.mkdirSync(dest, { recursive: true })

  const wanted = new Set(fs.readdirSync(src))
  for (const name of fs.readdirSync(dest)) {
    if (!wanted.has(name)) {
      fs.rmSync(confine(DEST, dest, name), { recursive: true, force: true })
    }
  }

  for (const name of wanted) {
    const from = confine(SRC, src, name)
    const to = confine(DEST, dest, name)
    if (fs.statSync(from).isDirectory()) {
      sync(from, to)
    } else if (!fs.existsSync(to) || !fs.readFileSync(from).equals(fs.readFileSync(to))) {
      fs.copyFileSync(from, to)
    }
  }
}

if (!fs.existsSync(SRC)) {
  console.error(`original test suite not found at ${SRC}`)
  process.exit(1)
}

sync(SRC, DEST)
