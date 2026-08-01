'use strict'

// Synchronous bridge to the Rust `semver-rpc` binary.
//
// node-semver's API is entirely synchronous, so the adapter cannot use Node's
// asynchronous child-process streams: blocking the event loop to wait for a
// reply would deadlock. Instead the child is wired up to a pair of FIFOs, which
// `fs.openSync` opens in blocking mode. `fs.readSync` on a blocking FIFO parks
// the thread until the Rust process answers, which gives us real synchronous
// request/response with no polling and no per-call process spawn.
//
// One child is spawned per Node process, on first use, and reused for every
// call.

const { spawn, spawnSync } = require('child_process')
const fs = require('fs')
const os = require('os')
const path = require('path')

const DEFAULT_BIN = path.resolve(__dirname, '..', '..', 'target', 'release', 'semver-rpc')

let state = null

const binPath = () => process.env.SEMVER_RPC_BIN || DEFAULT_BIN

const start = () => {
  const bin = binPath()
  if (!fs.existsSync(bin)) {
    throw new Error(
      `semver-rpc binary not found at ${bin}\n` +
      'Build it with `cargo build --release` or set SEMVER_RPC_BIN.'
    )
  }

  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'semver-rpc-'))
  const reqPath = path.join(dir, 'req')
  const resPath = path.join(dir, 'res')
  const mk = spawnSync('mkfifo', [reqPath, resPath])
  if (mk.status !== 0) {
    throw new Error(`failed to create rpc fifos in ${dir}`)
  }

  // The shell opens the redirections in order: `req` for reading, then `res`
  // for writing. We open them in the mirrored order below so neither side
  // deadlocks waiting for its peer.
  const child = spawn('/bin/sh', ['-c', 'exec "$0" < "$1" > "$2"', bin, reqPath, resPath], {
    stdio: ['ignore', 'ignore', 'inherit'],
  })
  child.unref()

  const writeFd = fs.openSync(reqPath, 'w')
  const readFd = fs.openSync(resPath, 'r')

  // The FIFOs only needed to exist long enough for both ends to open them.
  fs.unlinkSync(reqPath)
  fs.unlinkSync(resPath)
  fs.rmdirSync(dir)

  state = {
    child,
    writeFd,
    readFd,
    buffer: Buffer.alloc(0),
    chunk: Buffer.allocUnsafe(1 << 16),
  }

  process.on('exit', () => {
    try {
      fs.closeSync(state.writeFd)
    } catch {
      // the child is going away with us either way
    }
    child.kill()
  })

  return state
}

const readLine = (s) => {
  for (;;) {
    const nl = s.buffer.indexOf(10)
    if (nl >= 0) {
      const line = s.buffer.subarray(0, nl).toString('utf8')
      s.buffer = s.buffer.subarray(nl + 1)
      return line
    }
    const n = fs.readSync(s.readFd, s.chunk, 0, s.chunk.length, null)
    if (n === 0) {
      throw new Error('semver-rpc closed unexpectedly')
    }
    const read = s.chunk.subarray(0, n)
    s.buffer = s.buffer.length ? Buffer.concat([s.buffer, read]) : Buffer.from(read)
  }
}

const writeAll = (s, buf) => {
  let off = 0
  while (off < buf.length) {
    off += fs.writeSync(s.writeFd, buf, off, buf.length - off)
  }
}

// Send one request and return the decoded envelope: { ok, result } or
// { ok: false, error, name }.
const send = (op, args) => {
  const s = state || start()
  writeAll(s, Buffer.from(`${JSON.stringify({ op, args })}\n`, 'utf8'))
  return JSON.parse(readLine(s))
}

const ERRORS = { TypeError, Error }

// Send a request, throwing the JS error the original would have thrown.
const call = (op, ...args) => {
  const res = send(op, args)
  if (res.ok) {
    return res.result
  }
  const Ctor = ERRORS[res.name] || TypeError
  throw new Ctor(res.error)
}

// Send a request, returning `fallback` instead of throwing.
const tryCall = (fallback, op, ...args) => {
  const res = send(op, args)
  return res.ok ? res.result : fallback
}

module.exports = { call, tryCall, send, binPath }
