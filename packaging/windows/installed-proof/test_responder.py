import json
import threading
import unittest
from urllib.error import HTTPError
from urllib.request import Request, urlopen

import responder


class ResponderTests(unittest.TestCase):
    def request(self, messages, tools=()):
        return {'messages': messages, 'tools': list(tools)}

    def test_auxiliary_requests_do_not_call_tools(self):
        self.assertEqual(responder.choose_reply(self.request([{'role': 'user', 'content': 'title'}]))[1], 'stop')

    def test_proof_discovers_advertised_name_and_sends_wait(self):
        request = self.request([{'role': 'user', 'content': responder.PROMPT}],
                               [{'function': {'name': 'ivy_panel_set'}}])
        delta, reason = responder.choose_reply(request)
        call = delta['tool_calls'][0]['function']
        self.assertEqual((call['name'], reason), ('ivy_panel_set', 'tool_calls'))
        self.assertEqual(json.loads(call['arguments'])['document']['attention'], 'wait')

    def test_missing_or_ambiguous_tool_is_failure(self):
        messages = [{'role': 'user', 'content': responder.PROMPT}]
        for names in ([], ['ivy_panel_set', 'other_panel_set']):
            with self.subTest(names=names), self.assertRaises(ValueError):
                responder.choose_reply(self.request(messages, [{'function': {'name': name}} for name in names]))

    def test_proof_call_cannot_repeat_for_the_same_nonce(self):
        request = self.request([{'role': 'user', 'content': responder.PROMPT + 'abc'}],
                               [{'function': {'name': 'ivy_panel_set'}}])
        state = {'lock': threading.Lock(), 'called': False}
        responder.choose_reply(request, 'abc', state=state)
        with self.assertRaisesRegex(ValueError, 'already requested'):
            responder.choose_reply(request, 'abc', state=state)

    def test_tool_result_requests_readback_then_finishes(self):
        messages = [{'role': 'user', 'content': responder.PROMPT}, {'role': 'tool', 'content': 'ok'}]
        request = self.request(messages, [{'function': {'name': 'ivy_panel_get'}}])
        delta, reason = responder.choose_reply(request)
        self.assertEqual((delta['tool_calls'][0]['function']['name'], reason), ('ivy_panel_get', 'tool_calls'))
        messages.append({'role': 'tool', 'content': 'document'})
        self.assertEqual(responder.choose_reply(self.request(messages))[1], 'stop')

    def test_stream_has_terminal_marker(self):
        data = b''.join(responder.chunks(self.request([{'role': 'user', 'content': 'title'}])))
        self.assertTrue(data.endswith(b'data: [DONE]\n\n'))
        self.assertIn(b'"finish_reason": "stop"', data)

    def test_server_rejects_other_endpoints(self):
        import tempfile
        from pathlib import Path
        with tempfile.TemporaryDirectory() as directory:
            try:
                server = responder.serve(Path(directory) / 'requests.jsonl')
            except PermissionError:
                self.skipTest('sandbox disallows loopback sockets')
            thread = threading.Thread(target=server.serve_forever)
            thread.start()
            try:
                url = f'http://127.0.0.1:{server.server_port}/other'
                with self.assertRaises(HTTPError) as caught:
                    urlopen(Request(url, data=b'{}'), timeout=3)
                self.assertEqual(caught.exception.code, 400)
                caught.exception.close()
            finally:
                server.shutdown()
                thread.join()
                server.server_close()


if __name__ == '__main__':
    unittest.main()
