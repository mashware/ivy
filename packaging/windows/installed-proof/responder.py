"""Loopback-only, deterministic OpenAI-compatible model for the installed-client trial."""
import json
import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

PROMPT = 'IVY_INSTALLED_PROOF_REQUEST'
WAIT_LINE = 'Installed OpenCode wait witness'


def choose_reply(request, nonce='', tool_name_path=None, state=None):
    """Return one tool call only when the explicit proof request reaches the model."""
    messages = request.get('messages', [])
    serialized = json.dumps(messages, ensure_ascii=False)
    if PROMPT + nonce not in serialized:
        return {'content': 'Ivy proof'}, 'stop'
    tool_results = sum(message.get('role') == 'tool' for message in messages)
    if tool_results:
        if tool_results >= 2:
            return {'content': 'Ivy proof complete'}, 'stop'
        names = [tool.get('function', {}).get('name', '') for tool in request.get('tools', [])]
        matches = [name for name in names if name.lower().endswith('panel_get')]
        if len(matches) != 1:
            raise ValueError(f'expected exactly one advertised panel_get tool; got {matches!r}')
        if tool_name_path is not None:
            Path(str(tool_name_path) + '-get').write_text(matches[0], encoding='utf-8')
        return {'tool_calls': [{'index': 0, 'id': 'call_ivy_readback', 'type': 'function',
                                'function': {'name': matches[0], 'arguments': '{}'}}]}, 'tool_calls'
    names = [tool.get('function', {}).get('name', '') for tool in request.get('tools', [])]
    matches = [name for name in names if name.lower().endswith('panel_set')]
    if len(matches) != 1:
        raise ValueError(f'expected exactly one advertised panel_set tool; got {matches!r}')
    if state is not None:
        with state['lock']:
            if state['called']:
                raise ValueError('proof tool was already requested')
            state['called'] = True
    if tool_name_path is not None:
        Path(tool_name_path).write_text(matches[0], encoding='utf-8')
    arguments = {'source': 'flow', 'document': {'lines': [WAIT_LINE + (' ' + nonce if nonce else '')], 'phase': 'build', 'attention': 'wait'}}
    return {'tool_calls': [{'index': 0, 'id': 'call_ivy_proof', 'type': 'function',
                            'function': {'name': matches[0], 'arguments': json.dumps(arguments)}}]}, 'tool_calls'


def chunks(request, nonce='', tool_name_path=None, state=None):
    delta, reason = choose_reply(request, nonce, tool_name_path, state)
    base = {'id': 'chatcmpl-ivy-proof', 'object': 'chat.completion.chunk', 'created': 1,
            'model': 'scripted'}
    for value, finish in [({'role': 'assistant'}, None), (delta, None), ({}, reason)]:
        chunk = dict(base, choices=[{'index': 0, 'delta': value, 'finish_reason': finish}])
        yield ('data: ' + json.dumps(chunk, ensure_ascii=False) + '\n\n').encode('utf-8')
    yield b'data: [DONE]\n\n'


def make_handler(transcript, nonce='', tool_name_path=None):
    state = {'lock': threading.Lock(), 'called': False}
    class Handler(BaseHTTPRequestHandler):
        def log_message(self, *_args):
            pass

        def do_POST(self):
            try:
                if self.path != '/v1/chat/completions':
                    raise ValueError(f'unexpected path {self.path!r}')
                length = int(self.headers.get('Content-Length', '0'))
                if not 0 < length <= 2_000_000:
                    raise ValueError('invalid request size')
                request = json.loads(self.rfile.read(length))
                response = b''.join(chunks(request, nonce, tool_name_path, state))
                with Path(transcript).open('a', encoding='utf-8') as log:
                    log.write(json.dumps({'path': self.path, 'request': request,
                                          'response': response.decode()}, ensure_ascii=False) + '\n')
                self.send_response(200)
                self.send_header('Content-Type', 'text/event-stream')
                self.send_header('Content-Length', str(len(response)))
                self.end_headers()
                self.wfile.write(response)
            except Exception as error:
                with Path(transcript).open('a', encoding='utf-8') as log:
                    log.write(json.dumps({'error': str(error)}) + '\n')
                self.send_error(400, str(error))
    return Handler


def serve(transcript, nonce='', tool_name_path=None):
    return ThreadingHTTPServer(('127.0.0.1', 0), make_handler(transcript, nonce, tool_name_path))
