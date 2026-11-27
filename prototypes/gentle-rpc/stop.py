"""Run two installed Gentle launchers against a deterministic loopback model (Linux)."""

import argparse
import asyncio
from contextlib import AsyncExitStack, asynccontextmanager
import json
import os
from pathlib import Path
import signal
import subprocess
from tempfile import TemporaryDirectory

SCHEMA = "gentle-shell.desktop-rpc/v1"
AGENT = "ivy-stop-reader"


class Model:
    """Parents call the real subagent tool. Each child's HTTP stream stays open."""

    def __init__(self, background):
        self.background = background
        self.started = {name: asyncio.Event() for name in "AB"}
        self.disconnected_a = asyncio.Event()
        self.release_b = asyncio.Event()

    async def serve(self, reader, writer):
        waiting_a = False
        try:
            headers = (await reader.readuntil(b"\r\n\r\n")).decode().splitlines()
            length = int(next(h for h in headers if h.lower().startswith("content-length:")).split(":")[1])
            request = json.loads(await reader.readexactly(length))
            last = request["messages"][-1]
            content = last["content"]
            if isinstance(content, list):
                content = "".join(part.get("text", "") for part in content)
            writer.write(b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n")
            await writer.drain()
            reason = "stop"
            if last["role"] == "user" and content in ("parent A", "parent B"):
                name = content[-1]
                args = {"agent": AGENT, "task": f"child {name}", "mode": "background" if self.background else "task"}
                delta = {"role": "assistant", "tool_calls": [{
                    "index": 0, "id": f"call_{name}", "type": "function",
                    "function": {"name": "subagent_run", "arguments": json.dumps(args)},
                }]}
                if self.background:
                    job = {"command": "sleep 90", "label": "Probe job " + name}
                    delta["tool_calls"].append({"index":1, "id":"job_call_" + name,"type":"function",
                        "function":{"name":"bash_background","arguments":json.dumps(job)}})
                reason = "tool_calls"
            elif last["role"] == "user" and content in ("child A", "child B"):
                name = content[-1]
                self.started[name].set()
                if name == "A":
                    waiting_a = True
                    # This stream never finishes on its own. Only its client can end it.
                    if await reader.read(1) != b"":
                        raise AssertionError("Unexpected pipelined request from A")
                    self.disconnected_a.set()
                    return
                await self.release_b.wait()
                delta = {"role": "assistant", "content": "B reached its own completion"}
            elif self.background and last["role"] == "tool":
                # Keep the parent in a real turn after background delegation returns.
                await reader.read(1)
                return
            else:
                delta = {"role": "assistant", "content": "Parent finished"}
            for chunk, finish in [(delta, None), ({}, reason)]:
                record = {"id": "fixture", "object": "chat.completion.chunk", "created": 1700000000,
                          "model": "fixture", "choices": [{"index": 0, "delta": chunk, "finish_reason": finish}]}
                writer.write(("data: " + json.dumps(record) + "\n\n").encode())
            writer.write(b"data: [DONE]\n\n")
            await writer.drain()
        except (BrokenPipeError, ConnectionResetError):
            # Clients can close a request before awaiting any response body.
            if waiting_a:
                self.disconnected_a.set()
        finally:
            writer.close()


class Host:
    def __init__(self, process):
        self.process = process
        self.sequence = 0

    async def prompt(self, message):
        self.process.stdin.write((json.dumps({"type": "prompt", "message": message}) + "\n").encode())
        await self.process.stdin.drain()

    async def call(self, method, **params):
        self.sequence += 1
        request_id = f"ivy-{self.sequence}"
        request = {"schema": SCHEMA, "requestId": request_id, "method": method, "params": params}
        await self.prompt("/gentle:desktop " + json.dumps(request))
        async with asyncio.timeout(20):
            while line := await self.process.stdout.readline():
                record = json.loads(line)
                if record.get("type") != "extension_ui_request" or record.get("method") != "setWidget":
                    continue
                if record.get("widgetKey") != "gentle-desktop-rpc":
                    continue
                for item in record.get("widgetLines", []):
                    envelope = json.loads(item)
                    if (envelope.get("schema"), envelope.get("kind"), envelope.get("requestId")) == (SCHEMA, "response", request_id):
                        return envelope
        raise RuntimeError(f"Gentle exited before answering {method}")

    async def native(self, command):
        self.sequence += 1
        request_id = f"native-{self.sequence}"
        self.process.stdin.write((json.dumps({"type":command,"id":request_id}) + "\n").encode())
        await self.process.stdin.drain()
        async with asyncio.timeout(20):
            while line := await self.process.stdout.readline():
                answer = json.loads(line)
                if answer.get("type") == "response" and answer.get("id") == request_id:
                    assert answer.get("command") == command and answer.get("success"), answer
                    return answer.get("data")
        raise RuntimeError(f"Gentle exited before answering {command}")

    async def agents(self):
        answer = await self.call("list_subagents")
        assert answer["ok"], answer
        return answer["data"]["agents"]

    async def expect(self, task, state):
        async with asyncio.timeout(20):
            while True:
                rows = await self.agents()
                if any(row["id"] == task and row["status"] == state for row in rows):
                    return
                await asyncio.sleep(0.1)


@asynccontextmanager
async def session(node, launcher, home, port):
    agent_home = home / ".pi" / "agent"
    definitions = agent_home / "agents"
    definitions.mkdir(parents=True)
    (definitions / f"{AGENT}.md").write_text(f"---\nname: {AGENT}\ndescription: Test reader\ntools: read\n---\nRead only.\n")
    provider = {
        "baseUrl": f"http://127.0.0.1:{port}/v1", "api": "openai-completions", "apiKey": "fixture",
        "models": [{"id": "fixture", "name": "fixture", "reasoning": False, "input": ["text"],
                    "contextWindow": 64000, "maxTokens": 4096}],
    }
    (agent_home / "models.json").write_text(json.dumps({"providers": {"ivy-local": provider}}))
    env = {"HOME": str(home), "PATH": f"{Path(node).parent}:/usr/bin:/bin", "PI_OFFLINE": "1",
           "PI_CODING_AGENT_DIR": str(agent_home), "GENTLE_PI_AGENT_HOME": str(agent_home),
           "GENTLE_SHELL_INTERACTIVE_HOST": "1"}
    with (home / "stderr").open("wb") as stderr:
        process = await asyncio.create_subprocess_exec(
            node, launcher, "--link", "--mode", "rpc", "--model", "ivy-local/fixture",
            cwd=home, env=env, stdin=asyncio.subprocess.PIPE, stdout=asyncio.subprocess.PIPE,
            stderr=stderr, start_new_session=True, limit=2_000_000,
        )
        try:
            yield Host(process)
        finally:
            if process.returncode is None:
                os.killpg(process.pid, signal.SIGTERM)
                try:
                    await asyncio.wait_for(process.wait(), 5)
                except TimeoutError:
                    os.killpg(process.pid, signal.SIGKILL)
                    await asyncio.wait_for(process.wait(), 5)


async def main(installation, background):
    node = subprocess.check_output(["node", "-p", "process.execPath"], text=True).strip()
    launcher = installation / "node_modules/gentle-pi/bin/gentle-shell.mjs"
    model = Model(background)
    with TemporaryDirectory(prefix="ivy-gentle-stop-") as directory:
        async with AsyncExitStack() as stack:
            server = await stack.enter_async_context(await asyncio.start_server(model.serve, "127.0.0.1", 0))
            port = server.sockets[0].getsockname()[1]
            a, b = [await stack.enter_async_context(session(node, launcher, Path(directory) / name, port)) for name in "AB"]
            stack.callback(model.release_b.set)
            original = (await a.native("get_state"))["sessionId"]
            for name, host in [("A", a), ("B", b)]:
                capabilities = await host.call("capabilities")
                assert {"cancel_subagent", "list_subagents"} <= set(capabilities["data"]["methods"])
                assert await host.agents() == []
                await host.prompt(f"parent {name}")
            # Drain pre-existing events using the RPC path so large startup prompts do not fill stdout.
            async with asyncio.timeout(60):
                while not all(event.is_set() for event in model.started.values()):
                    await asyncio.gather(a.agents(), b.agents())
                    await asyncio.sleep(0.1)
            [first], [second] = await asyncio.gather(a.agents(), b.agents())
            assert first["agent"] == second["agent"] == AGENT
            assert first["status"] == second["status"] == "running"
            foreign = await b.call("cancel_subagent", id=first["id"])
            assert foreign["ok"] is False and foreign["error"]["code"] == "not_found", foreign
            if background:
                [job_a], [job_b] = [(await host.call("list_jobs"))["data"]["jobs"] for host in (a,b)]
                assert job_a["status"] == job_b["status"] == "running"
                await a.native("abort")
                await a.expect(first["id"], "running")
                assert not model.disconnected_a.is_set()
                print("After the acknowledged parent abort, A's background helper is still running.")
            result = await a.call("cancel_subagent", id=first["id"])
            assert result["ok"] and result["data"]["agent"]["id"] == first["id"], result
            print("A's immediate cancel response:", result["data"]["agent"]["status"])
            await asyncio.gather(a.expect(first["id"], "cancelled"), asyncio.wait_for(model.disconnected_a.wait(), 10))
            await b.expect(second["id"], "running")
            if background:
                job = (await a.call("cancel_job", id=job_a["id"]))["data"]["job"]
                assert (job["id"],job["status"]) == (job_a["id"],"stopped")
                [neighbor] = (await b.call("list_jobs"))["data"]["jobs"]
                assert (neighbor["id"],neighbor["status"]) == (job_b["id"],"running")
                print("A's background job stopped while B's job remained running.")
                assert (await b.call("cancel_job", id=job_b["id"]))["data"]["job"]["status"] == "stopped"
            model.release_b.set()
            await b.expect(second["id"], "completed")
            async with asyncio.timeout(15):
                while (await a.native("get_state"))["isStreaming"]:
                    await asyncio.sleep(0.1)
            await a.prompt("follow-up A")
            async with asyncio.timeout(15):
                while True:
                    state = await a.native("get_state")
                    messages = (await a.native("get_messages"))["messages"]
                    user_text = [str(m.get("content")) for m in messages if m["role"] == "user"]
                    if (not state["isStreaming"] and any("follow-up A" in m for m in user_text)
                            and messages[-1]["role"] == "assistant"):
                        break
                    await asyncio.sleep(0.1)
            assert state["sessionId"] == original
            assert any("parent A" in m for m in user_text)
            print("The next prompt finished in the same conversation, with the original request still present.")
            print("A's owner confirmed cancellation and A's child closed its model stream. B completed independently.")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("installation", type=Path)
    parser.add_argument("--background", action="store_true", help="abort the parent before stopping its background helper")
    args = parser.parse_args()
    asyncio.run(main(args.installation.resolve(), args.background))
