import { test } from 'node:test'
import assert from 'node:assert/strict'
import { mkdtemp, mkdir, readFile, writeFile, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { pathToFileURL } from 'node:url'
import { createServer } from 'node:net'

const pause = ms => new Promise(r => setTimeout(r, ms))
test('route bootstrap, displayed request, cancellation, navigation and disposal', async () => {
  const dir = await mkdtemp(join(tmpdir(), 'ivy-opencode-'))
  await mkdir(join(dir, 'node_modules/solid-js'), {recursive:true})
  await writeFile(join(dir, 'node_modules/solid-js/package.json'), '{"type":"module","exports":"./index.js"}')
  await writeFile(join(dir, 'node_modules/solid-js/index.js'), `let effects=[]; export function createRoot(f){return f(()=>{effects=[]})} export function createSignal(v){return [()=>v,x=>{v=typeof x==='function'?x(v):x; for(const e of effects)queueMicrotask(e)}]} export function createEffect(f){effects.push(f);f()} export function tick(){for(const e of effects)queueMicrotask(e)}`)
  await writeFile(join(dir, 'net.mjs'), `import {EventEmitter} from 'node:events'; export const lines=[]; export function createConnection(){ const s=new EventEmitter(); s.connecting=true; s.write=x=>{lines.push(JSON.parse(x));queueMicrotask(()=>s.emit('data',Buffer.from('{"ok":true}\\n')))}; s.end=()=>s.emit('close'); s.destroy=s.end; queueMicrotask(()=>{s.connecting=false;s.emit('connect')}); return s }`)
  const source = await readFile(new URL('./ivy-opencode.tsx', import.meta.url), 'utf8')
  await writeFile(join(dir, 'plugin.ts'), process.env.IVY_REAL_SOCKET ? source : source.replace('from "node:net"', 'from "./net.mjs"'))
  const solid = await import(pathToFileURL(join(dir, 'node_modules/solid-js/index.js')))
  const plugin = (await import(pathToFileURL(join(dir, 'plugin.ts')))).default
  let server
  let lines
  if (process.env.IVY_REAL_SOCKET) {
    lines = []
    server = createServer(socket => { let buffer=''; socket.on('data', chunk => { buffer += chunk; let end; while((end=buffer.indexOf('\n')) >= 0) { lines.push(JSON.parse(buffer.slice(0,end))); buffer=buffer.slice(end+1); socket.write('{"ok":true}\n') } }) })
    await new Promise((resolve, reject) => {server.once('error',reject); server.listen(join(dir,'control.sock'),resolve)})
  } else ({lines} = await import(pathToFileURL(join(dir, 'net.mjs'))))
  process.env.PANEL_DIR=dir; process.env.PANEL_ID='test'
  let route={name:'session',params:{sessionID:'root'}}, pending=[], visible=[], dispose
  const callbacks = new Map()
  const api={route:{get current(){return route}},state:{ready:true,session:{get:()=>({id:'root',directory:dir}),status:()=>({type:'busy'}),permission:()=>[],question:()=>visible}},client:{session:{children:async()=>({data:[]})},permission:{list:async()=>({data:[]})},question:{list:async()=>({data:pending})}},event:{on:(name,cb)=>{callbacks.set(name,cb);return()=>callbacks.delete(name)}},lifecycle:{onDispose:fn=>dispose=fn}}
  try {
    await plugin.tui(api); await pause(80)
    assert.equal(lines.at(-1).snapshot.question,false)
    pending=[{id:'q',sessionID:'root'}]; visible=[...pending]; callbacks.get('question.asked')(); await pause(80)
    assert.equal(lines.at(-1).snapshot.question,true)
    api.client.question.list = async () => { throw new Error('offline') }
    callbacks.get('session.status')(); await pause(80)
    assert.equal(lines.at(-1).snapshot.stale,true)
    assert.equal(lines.at(-1).snapshot.question,true, 'failure keeps the wait')
    api.client.question.list = async () => ({data:pending})
    callbacks.get('session.status')(); await pause(80)
    assert.equal(lines.at(-1).snapshot.question,true)
    assert.notEqual(lines.at(-1).snapshot.stale,true, 'complete refresh restores current validity')
    pending=[]; callbacks.get('question.rejected')(); await pause(80)
    assert.equal(lines.at(-1).snapshot.question,false)
    let release, active = 0, peak = 0
    api.client.session.children = () => { active++; peak = Math.max(peak, active); return new Promise(resolve => { release = () => { active--; resolve({data:[]}) } }) }
    route={name:'session',params:{sessionID:'old'}}; solid.tick(); await pause(40)
    assert.equal(lines.at(-1).snapshot.stale,true, 'route bootstrap marks the prior claim stale immediately')
    assert.equal(lines.at(-1).snapshot.session,'root')
    for (let i=0; i<100; i++) callbacks.get('session.status')()
    await pause(40)
    assert.equal(peak, 1, 'an event storm must not launch concurrent refreshes')
    route={name:'home'}; solid.tick(); await pause(40)
    assert.equal(lines.at(-1).snapshot,null)
    route={name:'session',params:{sessionID:'latest'}}; solid.tick()
    api.client.session.children = async () => ({data:[]})
    release(); await pause(80)
    assert.equal(lines.at(-1).snapshot.session,'latest', 'late bootstrap must not revive an old route')
    route={name:'home'}; solid.tick(); await pause(40)
    await dispose(); assert.equal(lines.at(-1).snapshot,null)
  } finally { if (server) await new Promise(resolve => server.close(resolve)); delete process.env.PANEL_DIR; delete process.env.PANEL_ID; await rm(dir,{recursive:true,force:true}) }
})
