import { spawn } from 'node:child_process'
import { readFile, writeFile, rename } from 'node:fs/promises'
import path from 'node:path'

const sleep = ms => new Promise(resolve => setTimeout(resolve, ms))
async function record(name, data) {
  const target = path.join(process.env.IVY_PROOF_DIR, name)
  await writeFile(`${target}.tmp`, JSON.stringify(data))
  await rename(`${target}.tmp`, target)
}

function accepted(input, result, name, nonce) {
  return input?.tool === name && input.args?.source === 'flow' &&
    input.args?.document?.phase === 'build' && input.args?.document?.attention === 'wait' &&
    JSON.stringify(input.args?.document?.lines) === JSON.stringify([`Installed OpenCode wait witness ${nonce}`]) &&
    result?.isError !== true && Array.isArray(result?.content) && result.content.length === 1 &&
    result.content[0]?.type === 'text' && result.content[0]?.text === 'published'
}

async function publish() {
  const helper = process.env.IVY_SESSION_HELPER
  if (!helper || path.resolve(helper).toLowerCase() !== path.resolve(process.env.IVY_PROOF_HELPER).toLowerCase()) {
    throw new Error('Ivy did not inject the expected session helper')
  }
  const child = spawn(helper, ['publish'], { shell: false, windowsHide: true, stdio: ['pipe', 'pipe', 'pipe'] })
  let stdout = '', stderr = '', overflow = false, timedOut = false, spawnError = ''
  const collect = (stream, append) => stream.on('data', data => {
    append(data.toString('utf8'))
    if (Buffer.byteLength(stdout) + Buffer.byteLength(stderr) > 65536) {
      overflow = true
      child.kill()
    }
  })
  collect(child.stdout, value => { stdout = (stdout + value).slice(0, 65536) })
  collect(child.stderr, value => { stderr = (stderr + value).slice(0, 65536) })
  child.stdin.on('error', () => {})
  child.stdin.end(JSON.stringify({ lines: ['Installed OpenCode hook witness'], phase: 'build' }))
  const outcome = await new Promise(resolve => {
    let settled = false
    const finish = (code, signal) => {
      if (settled) return
      settled = true
      clearTimeout(deadline)
      clearTimeout(closeDeadline)
      resolve({ code, signal })
    }
    let closeDeadline
    const deadline = setTimeout(() => {
      timedOut = true
      child.kill()
      closeDeadline = setTimeout(() => finish(null, 'close-timeout'), 3000)
    }, 15000)
    child.on('error', error => { spawnError = String(error); finish(null, 'spawn-error') })
    child.on('close', finish)
  })
  const evidence = { pid: child.pid, ...outcome, stdout, stderr, overflow, timedOut, spawnError }
  await record('helper.json', evidence)
  let response
  try { response = JSON.parse(stdout) } catch { throw new Error('helper returned invalid JSON') }
  if (outcome.code !== 0 || response.ok !== true || overflow || timedOut || spawnError) {
    throw new Error('helper refused publication or did not finish safely')
  }
}

export const IvyInstalledProof = async () => ({
  'tool.execute.after': async (input, result) => {
    const dir = process.env.IVY_PROOF_DIR
    if (input.tool.toLowerCase().endsWith('panel_get')) {
      await record('readback.json', { input, result, pid: process.pid })
      return
    }
    const name = (await readFile(path.join(dir, 'tool-name'), 'utf8')).trim()
    if (input.tool !== name) return
    if (!accepted(input, result, name, process.env.IVY_PROOF_NONCE)) {
      await record('hook-error.json', { input, result })
      throw new Error('panel_set did not return the exact expected publication')
    }
    await record('gate-ready.json', { input, result, pid: process.pid })
    const deadline = Date.now() + 120000
    while (true) {
      try { await readFile(path.join(dir, 'release')); break } catch (error) {
        if (error.code !== 'ENOENT') throw error
      }
      if (Date.now() >= deadline) throw new Error('proof gate timed out')
      await sleep(100)
    }
    await publish()
    // Queue admission precedes the app's apply; give the real UI time to consume it.
    await sleep(2000)
  },
})
