import os,json,subprocess,select,time,pathlib
w=pathlib.Path('/work/repo/.claude/work/69-codex-mobile-panel-routing')
e=dict(os.environ,CODEX_HOME='/tmp/ivy69-research/direct-home')
p=subprocess.Popen(['/tmp/ivy69-research/package/vendor/x86_64-unknown-linux-musl/bin/codex','app-server'],env=e,stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=open('/tmp/ivy69-research/direct.stderr','w'),text=True)
log=[];n=0
import threading,queue
q=queue.Queue()
def reader():
 for line in p.stdout:q.put(json.loads(line))
threading.Thread(target=reader,daemon=True).start()
def rpc(m,params):
 global n
 n+=1;p.stdin.write(json.dumps({'id':n,'method':m,'params':params})+'\n');p.stdin.flush();end=time.monotonic()+20
 while time.monotonic()<end:
  r=q.get(timeout=max(.1,end-time.monotonic()));log.append(r)
  if r.get('id')==n:
   if 'error' in r:raise RuntimeError(r)
   return r['result']
try:
 rpc('initialize',{'clientInfo':{'name':'ivy69-probe','version':'1'},'capabilities':{'experimentalApi':True}})
 p.stdin.write('{"method":"initialized"}\n');p.stdin.flush()
 threads=[]
 for identity in ['console-A','console-B']:
  config={'mcp_servers.ivy':{'command':'/usr/bin/python3','args':[str(w/'probes/mcp-env.py')],'env':{'PANEL_ID':identity,'PANEL_DIR':'/tmp/ivy69-research/panels'}}}
  r=rpc('thread/start',{'cwd':'/tmp/ivy69-research','config':config});tid=r['thread']['id'];threads.append(tid)
 for tid in threads:
  status=rpc('mcpServerStatus/list',{'threadId':tid});print('ivy connected',any(x['name']=='ivy' and x['runtimeStatus']=='connected' for x in status['data']))
  r=rpc('mcpServer/tool/call',{'threadId':tid,'server':'ivy','tool':'identity','arguments':{}});print('identity',json.dumps(r))
 r=rpc('thread/resume',{'threadId':threads[0],'config':{'mcp_servers.ivy.env.PANEL_ID':'console-C'}})
 r=rpc('mcpServer/tool/call',{'threadId':threads[0],'server':'ivy','tool':'identity','arguments':{}}); print('loaded resume identity',json.dumps(r))
finally:
 log=[x for x in log if 'content' in x.get('result',{}) or 'thread' in x.get('result',{})]
 (w/'probes/direct-results.json').write_text(json.dumps(log,indent=2)+'\n');p.terminate()
 try:p.wait(timeout=3)
 except subprocess.TimeoutExpired:p.kill();p.wait()
