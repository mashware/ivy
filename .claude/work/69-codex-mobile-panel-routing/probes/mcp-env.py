import sys,json,os
for line in sys.stdin:
 r=json.loads(line); m=r.get('method'); i=r.get('id')
 if i is None: continue
 if m=='initialize': v={'protocolVersion':'2024-11-05','capabilities':{'tools':{}},'serverInfo':{'name':'ivy69-env-probe','version':'1'}}
 elif m=='tools/list': v={'tools':[{'name':'identity','description':'Report synthetic probe identity','inputSchema':{'type':'object','properties':{}}}]}
 elif m=='tools/call': v={'content':[{'type':'text','text':json.dumps({k:os.environ.get(k) for k in ['PANEL_ID','PANEL_DIR']})}]}
 else: v={}
 print(json.dumps({'jsonrpc':'2.0','id':i,'result':v}),flush=True)
 if m=='initialize':
  with open('/tmp/ivy69-research/mcp-starts.jsonl','a') as f:f.write(json.dumps({k:os.environ.get(k) for k in ['PANEL_ID','PANEL_DIR']} | {'parent_pid':os.getppid()})+'\n')
