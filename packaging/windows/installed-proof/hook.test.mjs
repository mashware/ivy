import test from 'node:test'
import assert from 'node:assert/strict'
import { mkdtemp, readFile, rm, writeFile } from 'node:fs/promises'
import os from 'node:os'
import path from 'node:path'
import { IvyInstalledProof } from './hook.js'

test('only the exact actual MCP publication can arm the gate', async () => {
  const dir = await mkdtemp(path.join(os.tmpdir(), 'ivy-hook-test-'))
  const previous = { ...process.env }
  try {
    process.env.IVY_PROOF_DIR = dir
    process.env.IVY_PROOF_NONCE = 'nonce'
    process.env.IVY_PROOF_HELPER = path.join(dir, 'helper')
    delete process.env.IVY_SESSION_HELPER
    await writeFile(path.join(dir, 'tool-name'), 'ivy_panel_set')
    const hook = (await IvyInstalledProof())['tool.execute.after']
    const input = { tool: 'ivy_panel_set', args: { source: 'flow', document: {
      phase: 'build', attention: 'wait', lines: ['Installed OpenCode wait witness nonce'],
    } } }
    for (const result of [{ content: [{ type: 'text', text: 'refused' }] },
      { isError: true, content: [{ type: 'text', text: 'published' }] },
      { content: [{ type: 'text', text: 'published; dropped data' }] }, {}]) {
      await assert.rejects(hook(input, result))
      await assert.rejects(readFile(path.join(dir, 'gate-ready.json')))
    }
    await writeFile(path.join(dir, 'release'), 'yes')
    await assert.rejects(hook(input, { content: [{ type: 'text', text: 'published' }] }), /did not inject/)
    assert.equal(JSON.parse(await readFile(path.join(dir, 'gate-ready.json'))).result.content[0].text, 'published')
  } finally {
    process.env = previous
    await rm(dir, { recursive: true, force: true })
  }
})
