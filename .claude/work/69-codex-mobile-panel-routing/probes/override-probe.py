import os,pty,select,time,signal,json,fcntl,termios,struct
binary='/tmp/ivy69-research/package/vendor/x86_64-unknown-linux-musl/bin/codex'
pid,fd=pty.fork()
if pid==0:
 fcntl.ioctl(0,termios.TIOCSWINSZ,struct.pack('HHHH',30,110,0,0))
 os.chdir('/tmp/ivy69-research')
 for key in ['CODEX_CI','CODEX_THREAD_ID','CODEX_SESSION_ID','CODEX_TOOL_CALL_ID']:os.environ.pop(key,None)
 os.environ['CODEX_HOME']='/tmp/ivy69-research/ordinary-home';os.environ['TERM']='xterm-256color'
 os.execv(binary,[binary,'--no-alt-screen','-c','mcp_servers.ivy.env.PANEL_ID="console-override"'])
print('TUI pid',pid,flush=True)
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
open('/tmp/ivy69-research/override-output.bin','wb').write(output)
print('CLI probe output bytes',len(output))
