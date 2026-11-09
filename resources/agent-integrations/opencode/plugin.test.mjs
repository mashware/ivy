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
  await writeFile(join(dir, 'net.mjs'), `import {EventEmitter} from 'node:events'; export const lines=[]; export const state={connections:0,socket:null,refuse:false,retryable:false}; export function createConnection(){ const s=new EventEmitter(); state.connections++; state.socket=s; s.connecting=true; let closed=false; s.write=x=>{lines.push(JSON.parse(x));queueMicrotask(()=>{if(!closed)s.emit('data',Buffer.from(state.refuse?JSON.stringify(state.retryable ? {ok:false,retryable:true} : {ok:false})+'\\n':'{"ok":true}\\n'))})}; s.end=()=>{if(!closed){closed=true;queueMicrotask(()=>s.emit('close'))}}; s.destroy=s.end; queueMicrotask(()=>{s.connecting=false;s.emit('connect')}); return s }`)

  const source = await readFile(new URL('./ivy-opencode.tsx', import.meta.url), 'utf8')
  await writeFile(join(dir, 'plugin.ts'), process.env.IVY_REAL_SOCKET ? source : source.replace('from "node:net"', 'from "./net.mjs"'))
  const solid = await import(pathToFileURL(join(dir, 'node_modules/solid-js/index.js')))
  const plugin = (await import(pathToFileURL(join(dir, 'plugin.ts')))).default
  let server
  let lines, state
  if (process.env.IVY_REAL_SOCKET) {
    lines = []
    state = {connections:0, socket:null, refuse:false, retryable:false}
    server = createServer(socket => { state.connections++; state.socket=socket; let buffer=''; socket.on('data', chunk => { buffer += chunk; let end; while((end=buffer.indexOf('\n')) >= 0) { lines.push(JSON.parse(buffer.slice(0,end))); buffer=buffer.slice(end+1); socket.write(state.refuse ? JSON.stringify(state.retryable ? {ok:false,retryable:true} : {ok:false})+'\n' : '{"ok":true}\n') } }) })
    await new Promise((resolve, reject) => {server.once('error',reject); server.listen(join(dir,'control.sock'),resolve)})
  } else ({lines, state} = await import(pathToFileURL(join(dir, 'net.mjs'))))
  process.env.PANEL_DIR=dir; process.env.PANEL_ID='test'
  let route={name:'session',params:{sessionID:'root'}}, pending=[], visible=[], dispose, status='busy'
  let permissions=[], visiblePermissions=[], descendants=[], childRoute=false
  const callbacks = new Map()
  const api={route:{get current(){return route}},state:{ready:true,session:{get:()=>({id:route.params?.sessionID,directory:dir,...(childRoute?{parentID:'root'}:{})}),status:()=>({type:status}),permission:id=>visiblePermissions.filter(p=>p.sessionID===id),question:id=>visible.filter(q=>q.sessionID===id)}},client:{session:{children:async()=>({data:descendants})},permission:{list:async()=>({data:permissions})},question:{list:async()=>({data:pending})}},event:{on:(name,cb)=>{callbacks.set(name,cb);return()=>callbacks.delete(name)}},lifecycle:{onDispose:fn=>dispose=fn}}
  try {
    await plugin.tui(api); await pause(80)
    assert.equal(lines.at(-1).snapshot.question,false)
    descendants=[{id:'child',parentID:'root'},{id:'grandchild',parentID:'child'},{id:'other',parentID:'unrelated'}]
    permissions=[{id:'p-root',sessionID:'root'},{id:'p-child',sessionID:'child'},{id:'p-other',sessionID:'other'}]
    visiblePermissions=[...permissions]
    pending=[{id:'q-child',sessionID:'child'},{id:'q-grandchild',sessionID:'grandchild'}]; visible=[...pending]
    callbacks.get('permission.asked')(); await pause(80)
    assert.equal(lines.at(-1).snapshot.permission,true,'displayed root permissions reach the wire')
    assert.equal(lines.at(-1).snapshot.question,true,'a displayed direct-child question reaches the root wire snapshot')
    status='idle'; callbacks.get('session.status')(); await pause(80)
    assert.equal(lines.at(-1).snapshot.activity,'idle')
    assert.equal(lines.at(-1).snapshot.permission,true,'idle activity cannot answer a pending permission')
    assert.equal(lines.at(-1).snapshot.question,true,'idle activity cannot answer a pending question')
    permissions=permissions.filter(p=>p.id!=='p-root')
    callbacks.get('permission.replied')(); await pause(80)
    assert.equal(lines.at(-1).snapshot.permission,true,'answering one permission must retain the concurrent direct-child permission')
    permissions=permissions.filter(p=>p.id!=='p-child')
    callbacks.get('permission.replied')(); await pause(80)
    assert.equal(lines.at(-1).snapshot.permission,false,'unrelated-root permissions must not keep this root waiting')
    assert.equal(lines.at(-1).snapshot.question,true,'permission resolution must not clear a concurrent question')
    pending=pending.filter(q=>q.id!=='q-child')
    callbacks.get('question.rejected')(); await pause(80)
    assert.equal(lines.at(-1).snapshot.question,false,'grandchild requests are outside the displayed root/direct-child scope')
    childRoute=true; route={name:'session',params:{sessionID:'child'}}
    permissions=[{id:'p-child',sessionID:'child'}]; visiblePermissions=[...permissions]
    pending=[{id:'q-child',sessionID:'child'}]; visible=[...pending]
    solid.tick(); await pause(80)
    assert.equal(lines.at(-1).snapshot.session,'child')
    assert.equal(lines.at(-1).snapshot.permission,false,'a child route does not display root permission prompts')
    assert.equal(lines.at(-1).snapshot.question,false,'a child route does not display root question prompts')
    childRoute=false; route={name:'session',params:{sessionID:'root'}}
    permissions=[]; visiblePermissions=[]; descendants=[]; pending=[]; visible=[]; status='busy'
    solid.tick(); await pause(80)
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
    const oldConnections=state.connections, oldLines=lines.length
    status='idle'
    api.client.question.list = async () => { throw new Error('offline during reconnect') }
    state.socket.destroy()
    await pause(1250)
    assert.equal(state.connections,oldConnections+1,'a disconnected socket reconnects')
    assert.equal(lines.length,oldLines,'failed reconnect refresh must not send a cached stale first frame')
    api.client.question.list = async () => ({data:pending})
    callbacks.get('session.status')(); await pause(80)
    assert.equal(state.connections,oldConnections+1,'the fresh connection survived the failed refresh')
    assert.ok(lines.length>oldLines,'the new connection sends a fresh snapshot')
    assert.notEqual(lines.at(-1).snapshot.stale,true,'first snapshot on the new connection is current')
    assert.equal(lines.at(-1).snapshot.session,'latest')
    assert.equal(lines.at(-1).snapshot.activity,'idle','reconnect reads current state')
    route={name:'home'}; solid.tick(); await pause(40)
    assert.equal(lines.at(-1).snapshot,null)
    route={name:'session',params:{sessionID:'latest'}}; solid.tick(); await pause(80)
    state.refuse=true; state.retryable=true
    route={name:'home'}; solid.tick(); await pause(80)
    route={name:'session',params:{sessionID:'latest'}}; solid.tick(); await pause(80)
    state.refuse=false
    await pause(1150)
    assert.equal(lines.at(-1).snapshot.session,'latest','retry must refresh the current route after a rejected home withdrawal')
    assert.equal(lines.at(-1).snapshot.activity,'idle')
    state.refuse=true; state.retryable=true
    const transientConnections=state.connections
    const firstTransientLine=lines.length
    callbacks.get('session.status')(); await pause(8500)
    assert.equal(state.connections,transientConnections,'retryable refusals retain the same connection')
    assert.ok(lines.length-firstTransientLine >= 9,'more than eight retries were exercised')
    state.refuse=false
    const transientLines=lines.length
    await pause(1250)
    assert.equal(state.connections,transientConnections,'successful retry keeps the connection')
    assert.ok(lines.length>transientLines,'retry sends a complete snapshot')
    assert.equal(lines[transientLines].snapshot.session,'latest')
    assert.notEqual(lines[transientLines].snapshot.stale,true,'the reconnect first frame is current')
    state.refuse=true; state.retryable=false
    callbacks.get('session.status')(); await pause(80)
    const refusedConnections=state.connections, refusedLines=lines.length
    callbacks.get('session.status')()
    await pause(1250)
    assert.equal(state.connections,refusedConnections,'a refusal permanently stops reconnection')
    assert.equal(lines.length,refusedLines,'a refusal sends no more frames')
    await dispose(); dispose=undefined
  } finally { if (dispose) await dispose(); if (server) await new Promise(resolve => server.close(resolve)); delete process.env.PANEL_DIR; delete process.env.PANEL_ID; await rm(dir,{recursive:true,force:true}) }
})
