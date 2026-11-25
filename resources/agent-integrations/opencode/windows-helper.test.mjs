import {test} from 'node:test'
import assert from 'node:assert/strict'
import {mkdtemp, readFile, writeFile, rm} from 'node:fs/promises'
import {tmpdir} from 'node:os'
import {join} from 'node:path'
import {pathToFileURL} from 'node:url'

const pause = ms => new Promise(resolve => setTimeout(resolve, ms))
async function fixture(platform = 'win32') {
  const dir = await mkdtemp(join(tmpdir(), 'ivy-helper-'))
  const source = await readFile(new URL('./ivy-opencode.tsx', import.meta.url), 'utf8')
  const body = source.slice(source.indexOf('const envelope'), source.indexOf('export default'))
  await writeFile(join(dir, 'channel.ts'), `
import {EventEmitter} from 'node:events'
export const timers = new Map()
function setTimeout(callback,delay){const id=globalThis.setTimeout(()=>{timers.delete(id);callback()},delay);timers.set(id,{callback,delay});return id}
function clearTimeout(id){timers.delete(id);globalThis.clearTimeout(id)}
export function expire(delay){const entry=[...timers].find(([,v])=>v.delay===delay);if(!entry)throw Error('timer missing '+delay);const [id,{callback}]=entry;clearTimeout(id);callback()}
export const state = {children:[], lines:[], connections:0, refreshes:0, mode:'hold', retain:false}
const process = {platform:${JSON.stringify(platform)}, env:{IVY_SESSION_HELPER:'C:\\\\Program Files\\\\Ivy & tools\\\\ivy-session.exe'}, geteuid:()=>10}
const lstatSync = () => ({isDirectory:()=>true,uid:10,mode:0o700})
function spawn(path,args,options) {
 const c = new EventEmitter(); c.stdout = new EventEmitter(); c.stderr={resume(){}}; c.stdin=new EventEmitter()
 c.stdin.write = frame => {state.lines.push(JSON.parse(frame)); if(state.mode==='accept')queueMicrotask(()=>c.stdout.emit('data',Buffer.from('{"ok":true}\\n')))}
 c.stdin.end = () => {c.ended=true;if(!state.retain)queueMicrotask(()=>c.emit('close'))}
 c.kill = () => {c.killed=true;if(!state.retain)queueMicrotask(()=>c.emit('close'))}
 c.path=path;c.args=args;c.options=options;state.children.push(c);state.connections++;return c
}
function createConnection() {
 const c=spawn('',[],{});c.connecting=true;c.write=c.stdin.write;c.end=c.stdin.end;c.destroy=c.kill
 c.stdout.on('data',b=>c.emit('data',b));queueMicrotask(()=>{c.connecting=false;c.emit('connect')});return c
}
${body}
export const setPath = path => process.env.IVY_SESSION_HELPER=path
export const open = () => channel('private',()=>state.refreshes++)
`)
  const loaded = await import(pathToFileURL(join(dir, 'channel.ts')))
  return {...loaded, remove:()=>rm(dir,{recursive:true,force:true})}
}
const snapshot = session => ({session,activity:'busy',permission:false,question:true})
const reply = (c, value) => c.stdout.emit('data', Buffer.from(JSON.stringify(value)+'\n'))

test('Windows helper locator, fixed argv and authenticated readiness', async () => {
 const f=await fixture();let t
 try {
  for(const path of [undefined,'ivy-session.exe','C:relative.exe','\\root.exe','\\\\host\\tool.exe','\\\\?\\C:\\tool.exe','C:\\bad\n.exe','C:\\bad\u0085.exe','C:\\bad\u009f.exe','C:\\tool.cmd']) {
   f.setPath(path);t=f.open();assert.equal(f.state.children.length,0);await t.close()
  }
  const path='C:\\Program Files\\Ivy & Unicode-界\\helper.exe';f.setPath(path);t=f.open()
  const c=f.state.children[0];assert.equal(c.path,path);assert.deepEqual(c.args,['signals'])
  assert.deepEqual(c.options,{shell:false,windowsHide:true,stdio:['pipe','pipe','pipe']})
  assert.equal(t.send(snapshot('before')),false);assert.equal(f.state.refreshes,0)
  reply(c,{ok:true,ready:true,v:1});assert.equal(f.state.refreshes,1)
  assert.equal(t.send(snapshot('after')),true);assert.equal(f.state.lines[0].snapshot.session,'after')
  reply(c,{ok:true});f.state.mode='accept';t.send(null);await t.drain();await t.close()
 } finally {await t?.close();await f.remove()}
})

for(const platform of ['win32','linux']) test(`${platform} disposal preserves pending null through retryable refusals`,async()=>{
 const f=await fixture(platform);const t=f.open()
 try {
  const c=f.state.children[0];if(platform==='win32')reply(c,{ok:true,ready:true,v:1});else await pause(0)
  t.send(snapshot('wait'));t.beginDispose();t.send(null)
  reply(c,{ok:false,error:'paused',retryable:true})
  await pause(1050);assert.equal(f.state.lines.at(-1).snapshot,null,'pending null must reach same connection without outer refresh')
  reply(c,{ok:false,error:'paused',retryable:true});await pause(1050)
  assert.equal(f.state.lines.at(-1).snapshot,null,'refused null itself must be retried')
  assert.equal(f.state.connections,1);assert.equal(f.state.refreshes,1)
  reply(c,{ok:true});await t.drain();await t.close()
 }finally{await t.close();await f.remove()}
})

test('helper framing bounds bytes before parse, retains only latest, reconnects fresh',async()=>{
 const f=await fixture();let t=f.open()
 try {
  let c=f.state.children[0];reply(c,{ok:true,ready:true,v:1})
  t.send(snapshot('first'));for(let i=0;i<100;i++)t.send(snapshot('latest-'+i))
  assert.equal(f.state.lines.length,1);reply(c,{ok:true});assert.equal(f.state.lines.length,2)
  assert.equal(f.state.lines[1].snapshot.session,'latest-99');reply(c,{ok:true})
  c.stdout.emit('data',Buffer.from(' '.repeat(65536)));assert.ok(!c.killed,'exact cap can remain partial')
  c.stdout.emit('data',Buffer.from('界'));assert.equal(c.killed,true,'UTF-8 byte overflow terminates before LF')
  await pause(1050);assert.equal(f.state.children.length,2);c=f.state.children[1]
  assert.equal(f.state.lines.length,2,'old snapshot is never replayed')
  reply(c,{ok:true,ready:true,v:1});assert.equal(f.state.refreshes,2)
  assert.equal(f.state.lines.length,2);t.send(snapshot('fresh'));assert.equal(f.state.lines.at(-1).snapshot.session,'fresh')
  reply(c,{ok:false,error:'disabled'});await pause(1050);assert.equal(f.state.children.length,2,'permanent refusal stops')
 }finally{await t.close();await f.remove()}
})

test('helper cleanup has one total deadline and never respawns a live child',async()=>{
 const f=await fixture();const t=f.open()
 try {
  const c=f.state.children[0];reply(c,{ok:true,ready:true,v:1});t.send(snapshot('blocked'));t.beginDispose();t.send(null)
  f.state.retain=true;const started=Date.now(),deadline=started+4750
  await t.drain(deadline-750);await t.close(deadline)
  assert.ok(Date.now()-started<4950,'entire disposal remains inside five-second host grace')
  assert.equal(c.ended,true);assert.equal(c.killed,true)
  await pause(1050);assert.equal(f.state.children.length,1,'old live helper blocks replacement')
 }finally{f.state.retain=false;f.state.children[0]?.emit('close');await t.close();await f.remove()}
})


test('readiness and reply deadlines kill without concurrent replacement; disposal suppresses retry',async()=>{
 const f=await fixture();let t=f.open()
 try {
  let c=f.state.children[0];f.state.retain=true
  f.expire(5000);assert.equal(c.killed,true)
  assert.equal(f.state.children.length,1,'a kill request is not proof of child exit')
  f.state.retain=false;c.emit('close');f.expire(1000)
  c=f.state.children[1];reply(c,{ok:true,ready:true,v:1});t.send(snapshot('blocked'))
  f.expire(7000);assert.equal(c.killed,true);await pause(0)
  t.beginDispose();assert.equal(f.timers.size,0,'disposal cancels connection retry')
  assert.equal(t.send(snapshot('late')),false);await t.close()
 }finally{f.state.retain=false;f.state.children.at(-1)?.emit('close');await t.close();await f.remove()}
})

test('startup retryability and invalid readiness are protocol decisions',async()=>{
 for(const [frame,permanent]of [
  [{ok:false,ready:true,v:1,error:'unavailable',retryable:true},false],
  [{ok:false,ready:true,v:1,error:'invalid locator',retryable:false},true],
  [{ok:true,ready:true,v:2},false],
 ]) {
  const f=await fixture();const t=f.open()
  try {
   reply(f.state.children[0],frame);await pause(0)
   assert.equal(f.state.refreshes,0);assert.equal(f.state.lines.length,0)
   if(permanent)assert.equal(f.timers.size,0)
   else {f.expire(1000);assert.equal(f.state.children.length,2)}
  }finally{await t.close();await f.remove()}
 }
})


for(const platform of ['win32','linux']) test(`${platform} rejected withdrawal refreshes the active route after null retry`,async()=>{
 const f=await fixture(platform);const t=f.open()
 try {
  const c=f.state.children[0];if(platform==='win32')reply(c,{ok:true,ready:true,v:1});else await pause(0)
  t.send(null);reply(c,{ok:false,error:'paused',retryable:true})
  assert.equal(t.send(snapshot('new-route')),false,'blocked route snapshot must be reread after retry')
  f.expire(1000)
  assert.equal(f.state.lines.at(-1).snapshot,null)
  assert.equal(f.state.refreshes,2,'active controller must reread current route after retrying withdrawal')
  t.send(snapshot('fresh-route'));reply(c,{ok:true})
  assert.equal(f.state.lines.at(-1).snapshot.session,'fresh-route')
  reply(c,{ok:true});await t.drain()
 }finally{await t.close();await f.remove()}
})
