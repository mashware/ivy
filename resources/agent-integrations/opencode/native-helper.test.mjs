// Launched by the native listener fixture with two independently authenticated panes.
import {test} from 'node:test'
import assert from 'node:assert/strict'
import {mkdtemp,mkdir,readFile,writeFile,rm} from 'node:fs/promises'
import {tmpdir} from 'node:os'
import {join} from 'node:path'
import {pathToFileURL} from 'node:url'

const pause=ms=>new Promise(resolve=>setTimeout(resolve,ms))
function completedRecords(contents) {
 return contents.split('\n').slice(0,-1).map(line=>JSON.parse(line))
}
test('actual Windows plugin and native helper keep two pane signals independent',{
 skip: process.platform !== 'win32' || !process.env.IVY_TEST_PANES ? 'Requires the Windows native opencode-signals example fixture' : false,
},async()=>{
 const panes=JSON.parse(process.env.IVY_TEST_PANES)
 assert.equal(panes.length,2)
 assert.ok(process.env.IVY_TEST_HELPER)
 assert.ok(process.env.IVY_TEST_EVENTS)
 const saved=Object.fromEntries(['PANEL_ID','PANEL_DIR','PANEL_SERVER_PID','IVY_SESSION_HELPER'].map(key=>[key,process.env[key]]))
 const dir=await mkdtemp(join(tmpdir(),'ivy-native-plugin-'))
 const sessions=[]
 try {
  await mkdir(join(dir,'node_modules','solid-js'),{recursive:true})
  await writeFile(join(dir,'node_modules','solid-js','package.json'),'{"type":"module","exports":"./index.js"}')
  await writeFile(join(dir,'node_modules','solid-js','index.js'),'export const createRoot=f=>f(()=>{});export const createEffect=f=>f();export const createSignal=v=>[()=>v,x=>{v=x}]')
  await writeFile(join(dir,'plugin.ts'),await readFile(new URL('./ivy-opencode.tsx',import.meta.url)))
  const plugin=(await import(pathToFileURL(join(dir,'plugin.ts')))).default
  for(const [index,pane] of panes.entries()) {
   process.env.PANEL_ID=pane.id;process.env.PANEL_DIR=pane.dir;process.env.PANEL_SERVER_PID=String(pane.pid)
   process.env.IVY_SESSION_HELPER=process.env.IVY_TEST_HELPER
   const state={permission:false,question:false,status:'busy',dispose:undefined,callbacks:new Map()}
   const sessionID=`native-${index}`
   const requests=kind=>state[kind]?[{id:kind,sessionID}]:[]
   const api={route:{current:{name:'session',params:{sessionID}}},state:{ready:true,session:{get:()=>({id:sessionID,directory:pane.dir}),status:()=>({type:state.status}),permission:()=>requests('permission'),question:()=>requests('question')}},client:{session:{children:async()=>({data:[]})},permission:{list:async()=>({data:requests('permission')})},question:{list:async()=>({data:requests('question')})}},event:{on:(name,callback)=>{state.callbacks.set(name,callback);return()=>state.callbacks.delete(name)}},lifecycle:{onDispose:fn=>state.dispose=fn}}
   sessions.push(state);await plugin.tui(api)
  }
  async function observed(pane,predicate) {
   const deadline=Date.now()+12000
   while(Date.now()<deadline) {
    let rows=[]
    try {rows=completedRecords(await readFile(process.env.IVY_TEST_EVENTS,'utf8'))} catch(error) {if(error.code!=='ENOENT')throw error}
    if(rows.some(row=>row.pane===pane&&predicate(row.snapshot)))return
    await pause(25)
   }
   assert.fail(`native listener did not observe expected snapshot for pane ${pane}`)
  }
  for(let i=0;i<2;i++)await observed(i,s=>s?.activity==='busy'&&!s.permission&&!s.question)
  for(const state of sessions){state.permission=true;state.callbacks.get('permission.asked')()}
  for(let i=0;i<2;i++)await observed(i,s=>s?.permission===true)
  sessions[0].permission=false;sessions[0].status='idle';sessions[0].callbacks.get('permission.replied')()
  await observed(0,s=>s?.activity==='idle'&&s.permission===false)
  sessions[1].permission=false;sessions[1].status='idle';sessions[1].callbacks.get('permission.replied')()
  await observed(1,s=>s?.activity==='idle'&&s.permission===false)
  await Promise.all(sessions.map(async state=>{await state.dispose();state.dispose=undefined}))
  for(let i=0;i<2;i++)await observed(i,s=>s===null)
 }finally{
  await Promise.all(sessions.map(state=>state.dispose?.()))
  for(const [key,value]of Object.entries(saved)){if(value===undefined)delete process.env[key];else process.env[key]=value}
  await rm(dir,{recursive:true,force:true})
 }
})

test('completed signal records defer every unterminated write fragment',()=>{
 const first={pane:0,snapshot:{activity:'busy'}}
 const second={pane:1,snapshot:{permission:true}}
 const firstLine=JSON.stringify(first)+'\n'
 const secondLine=JSON.stringify(second)+'\n'
 for(let length=0;length<firstLine.length;length++) {
  assert.deepEqual(completedRecords(firstLine.slice(0,length)),[],`first record fragment ${length}`)
 }
 for(let length=0;length<secondLine.length;length++) {
  assert.deepEqual(completedRecords(firstLine+secondLine.slice(0,length)),[first],`second record fragment ${length}`)
 }
 assert.deepEqual(completedRecords(firstLine+secondLine),[first,second])
})

test('completed signal records reject malformed complete lines',()=>{
 const valid=JSON.stringify({pane:0,snapshot:null})+'\n'
 assert.deepEqual(completedRecords(valid+'{'),[{pane:0,snapshot:null}])
 assert.throws(()=>completedRecords(valid+'{\n'),SyntaxError)
 assert.throws(()=>completedRecords('\n'),SyntaxError)
})
