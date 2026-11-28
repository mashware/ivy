"""Capture real pinned Gentle/Pi conversation events against a local fixture model."""
import argparse
import asyncio
import shutil
import subprocess
from tempfile import TemporaryDirectory
from contextlib import AsyncExitStack, asynccontextmanager
import json
import os
from pathlib import Path
import signal

ROOT = None
HOME_ROOT = None
NODE = None
EVIDENCE = None
RUNTIME_LABEL = ''
SCHEMA = 'gentle-shell.desktop-rpc/v1'

async def model(reader, writer):
    try:
        headers = (await reader.readuntil(b'\r\n\r\n')).decode().splitlines()
        length = int(next(h for h in headers if h.lower().startswith('content-length:')).split(':')[1])
        request = json.loads(await reader.readexactly(length))
        messages = request['messages']
        name = 'B' if any('start B' in str(m.get('content')) for m in messages) else 'A'
        has_tool = any(m['role'] == 'tool' for m in messages)
        writer.write(b'HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n')
        if not has_tool:
            args = {'action':'write','tasks':[{'title':'Observe '+name,'status':'in_progress','note':'local model probe'}]}
            chunks = [{'role':'assistant','tool_calls':[{'index':0,'id':'todo-'+name,'type':'function','function':{'name':'todo','arguments':json.dumps(args)}}]}]
            finish = 'tool_calls'
        else:
            chunks = [{'role':'assistant','content':text} for text in ['First ', 'αβ🙂 ', 'answer for '+name]]
            finish = 'stop'
        for delta in chunks + [{}]:
            reason = finish if not delta else None
            event = {'id':'fixture','object':'chat.completion.chunk','created':1700000000,'model':'fixture','choices':[{'index':0,'delta':delta,'finish_reason':reason}]}
            writer.write(('data: '+json.dumps(event)+'\n\n').encode())
            await writer.drain()
            await asyncio.sleep(.01)
        writer.write(b'data: [DONE]\n\n')
        await writer.drain()
    finally:
        writer.close()

class Host:
    def __init__(self, process, name):
        self.process, self.name = process, name
        self.pending, self.events, self.serial = {}, [], 0
        self.settled = asyncio.Event()
        self.reader = asyncio.create_task(self.read())

    async def read(self):
        while line := await self.process.stdout.readline():
            value = json.loads(line)
            self.events.append(value)
            if value.get('type') == 'agent_settled':
                self.settled.set()
            if value.get('type') == 'response':
                future = self.pending.get(value.get('id'))
                if future is not None and not future.done():
                    future.set_result(value)
            if value.get('type') == 'extension_ui_request' and value.get('widgetKey') == 'gentle-desktop-rpc':
                for item in value.get('widgetLines',[]):
                    inner = json.loads(item)
                    future = self.pending.get(inner.get('requestId'))
                    if future is not None and not future.done():
                        future.set_result(inner)

    async def send(self, command, **fields):
        self.serial += 1
        ident = f'probe-{self.name}-{self.serial}'
        future = asyncio.get_running_loop().create_future()
        self.pending[ident] = future
        payload = dict(type=command,id=ident,**fields)
        self.process.stdin.write((json.dumps(payload)+'\n').encode())
        await self.process.stdin.drain()
        try:
            return await asyncio.wait_for(future,20)
        finally:
            self.pending.pop(ident,None)

    async def desktop(self, method, params):
        self.serial += 1
        ident = f'view-{self.name}-{self.serial}'
        future = asyncio.get_running_loop().create_future()
        self.pending[ident] = future
        request = dict(schema=SCHEMA,requestId=ident,method=method,params=params)
        try:
            await self.send('prompt',message='/gentle:desktop '+json.dumps(request))
            return await asyncio.wait_for(future,20)
        finally:
            self.pending.pop(ident,None)

@asynccontextmanager
async def session(name,port):
    home = HOME_ROOT / name
    agent = home / '.pi/agent'
    agent.mkdir(parents=True,exist_ok=True)
    provider = {'baseUrl':f'http://127.0.0.1:{port}/v1','api':'openai-completions','apiKey':'local-fixture','models':[{'id':'fixture','name':'fixture','reasoning':False,'input':['text'],'contextWindow':64000,'maxTokens':4096}]}
    (agent/'models.json').write_text(json.dumps({'providers':{'ivy-local':provider}}))
    env = {'HOME':str(home),'PATH':str(Path(NODE).parent)+':/usr/bin:/bin','PI_OFFLINE':'1','PI_CODING_AGENT_DIR':str(agent),'GENTLE_PI_AGENT_HOME':str(agent),'GENTLE_SHELL_INTERACTIVE_HOST':'1'}
    with (home/'stderr').open('wb') as errors:
        process = await asyncio.create_subprocess_exec(NODE,ROOT/'node_modules/gentle-pi/bin/gentle-shell.mjs','--link','--mode','rpc','--no-session','--model','ivy-local/fixture',cwd=home,env=env,stdin=asyncio.subprocess.PIPE,stdout=asyncio.subprocess.PIPE,stderr=errors,limit=4*1024*1024,start_new_session=True)
        host = Host(process,name)
        try:
            yield host
        finally:
            if process.returncode is None:
                os.killpg(process.pid,signal.SIGTERM)
                try:
                    await asyncio.wait_for(process.wait(),5)
                except TimeoutError:
                    os.killpg(process.pid,signal.SIGKILL)
                    await process.wait()
            await host.reader
            if EVIDENCE is not None:
                EVIDENCE.mkdir(parents=True,exist_ok=True)
                (EVIDENCE/('native-conversation-'+name+'.jsonl')).write_text(''.join(json.dumps(e)+'\n' for e in host.events))

async def main():
    async with AsyncExitStack() as stack:
        server = await stack.enter_async_context(await asyncio.start_server(model,'127.0.0.1',0))
        port = server.sockets[0].getsockname()[1]
        hosts = [await stack.enter_async_context(session(name,port)) for name in 'AB']
        original = [await h.send('get_state') for h in hosts]
        await asyncio.gather(*(h.send('prompt',message='start '+h.name) for h in hosts))
        await asyncio.gather(*(asyncio.wait_for(h.settled.wait(),30) for h in hosts))
        for host in hosts:
            assert any(e.get('type')=='tool_execution_start' and e.get('toolName')=='todo' for e in host.events)
            assert any(e.get('type')=='message_update' and e.get('assistantMessageEvent',{}).get('type')=='text_delta' for e in host.events)
            final = [e['message'] for e in host.events if e.get('type')=='message_end' and e.get('message',{}).get('role')=='assistant'][-1]
            text = ''.join(c.get('text','') for c in final['content'])
            assert text == 'First αβ🙂 answer for '+host.name,text
            snapshot = await host.desktop('get_snapshot',{'sections':['todo','activity']})
            assert snapshot['ok'],snapshot
            items = snapshot['data']['state']['session']['todo']['items']
            assert items[0]['title']=='Observe '+host.name and items[0]['state']=='in_progress',items
        a,b = hosts
        a.settled.clear()
        before_b = len(b.events)
        await a.send('prompt',message='follow-up A')
        await asyncio.wait_for(a.settled.wait(),30)
        state = await a.send('get_state')
        assert state['data']['sessionId'] == original[0]['data']['sessionId']
        assert not any(e.get('type')=='message_start' for e in b.events[before_b:])
        print('PASS ' + RUNTIME_LABEL + ': real delta/final/tool/todo snapshots and same-session follow-up; neighboring session isolated. Desktop not exercised.')

if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('installation', type=Path)
    parser.add_argument('--evidence-dir', type=Path)
    args = parser.parse_args()
    ROOT = args.installation.resolve()
    EVIDENCE = args.evidence_dir.resolve() if args.evidence_dir else None
    gentle_version = json.loads((ROOT/'node_modules/gentle-pi/package.json').read_text())['version']
    pi_version = json.loads((ROOT/'node_modules/@earendil-works/pi-coding-agent/package.json').read_text())['version']
    RUNTIME_LABEL = 'Gentle' + gentle_version + '/Pi' + pi_version
    NODE = subprocess.check_output([shutil.which('node') or 'node', '-p', 'process.execPath'], text=True).strip()
    with TemporaryDirectory(prefix='ivy-native-conversation-') as directory:
        HOME_ROOT = Path(directory)
        print('Isolated fixture home:', HOME_ROOT, flush=True)
        asyncio.run(main())
