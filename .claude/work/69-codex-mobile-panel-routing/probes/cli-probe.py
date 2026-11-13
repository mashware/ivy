import os,pty,select,time,signal,json
binary='/tmp/ivy69-research/package/vendor/x86_64-unknown-linux-musl/bin/codex'
pid,fd=pty.fork()
if pid==0:
 os.environ['CODEX_HOME']='/tmp/ivy69-research/client-home';os.environ['TERM']='xterm-256color'
 os.execv(binary,[binary,'--remote','ws://127.0.0.1:18769','-c','mcp_servers.ivy.command="/bin/false"','-c','mcp_servers.ivy.env.PANEL_ID="console-A"','-c','mcp_servers.ivy.env.PANEL_DIR="/tmp/ivy69-research/panels"','--no-alt-screen'])
end=time.monotonic()+12;output=b''
try:
 while time.monotonic()<end:
  if select.select([fd],[],[],.2)[0]:
   try:d=os.read(fd,65536)
   except OSError:break
   output+=d
   if b'\x1b[6n' in d:os.write(fd,b'\x1b[1;1R')
   if b'\x1b[c' in d:os.write(fd,b'\x1b[?1;2c')
finally:
 try:os.kill(pid,signal.SIGTERM)
 except ProcessLookupError:pass
 os.waitpid(pid,0)
open('/tmp/ivy69-research/cli-output.bin','wb').write(output)
print('CLI probe output bytes',len(output))
