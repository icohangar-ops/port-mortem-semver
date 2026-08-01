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

const sync = (src, dest) => {
  fs.mkdirSync(dest, { recursive: true })

  const wanted = new Set(fs.readdirSync(src))
  for (const name of fs.readdirSync(dest)) {
    if (!wanted.has(name)) {
      fs.rmSync(path.join(dest, name), { recursive: true, force: true })
    }
  }

  for (const name of wanted) {
    const from = path.join(src, name)
    const to = path.join(dest, name)
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
