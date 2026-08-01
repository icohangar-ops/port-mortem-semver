#!/usr/bin/env node
// Standalone semver comparison program.
//
// This is a shim: the real CLI is the Rust `semver` binary, which is exec'd
// with the same argv, stdio and exit status.

'use strict'

const { spawnSync } = require('child_process')
const fs = require('fs')
const path = require('path')

const bin = process.env.SEMVER_CLI_BIN ||
  path.resolve(__dirname, '..', '..', 'target', 'release', 'semver')

if (!fs.existsSync(bin)) {
  console.error(
    `semver binary not found at ${bin}\n` +
    'Build it with `cargo build --release` or set SEMVER_CLI_BIN.'
  )
  process.exit(1)
}

const res = spawnSync(bin, process.argv.slice(2), { stdio: 'inherit' })

if (res.error) {
  console.error(res.error.message)
  process.exit(1)
}

if (res.signal) {
  process.kill(process.pid, res.signal)
} else {
  process.exit(res.status)
}
