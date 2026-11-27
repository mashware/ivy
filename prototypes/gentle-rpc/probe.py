"""Run the Gentle RPC routing experiment with an isolated npm installation."""

import asyncio
from contextlib import AsyncExitStack, asynccontextmanager
import json
import os
from pathlib import Path
import select
import shutil
import signal
import subprocess
import sys
from tempfile import TemporaryDirectory


@asynccontextmanager
async def session(installation, node, pid_file=None):
    launcher = installation / "node_modules/gentle-pi/bin/gentle-shell.mjs"
    with TemporaryDirectory(prefix="ivy-gentle-session-") as directory:
        with (Path(directory) / "stderr").open("wb") as errors:
            process = await asyncio.create_subprocess_exec(
                node, launcher, "--link", "--mode", "rpc", "--no-session",
                "-e", installation / "probe.ts", "-e", installation / "delegated.ts",
                cwd=directory,
                env={
                    "HOME": directory,
                    "PATH": f"{Path(node).parent}:/usr/bin:/bin",
                    "GENTLE_SHELL_INTERACTIVE_HOST": "1",
                    "PI_OFFLINE": "1",
                    **({"IVY_PROBE_PID_FILE": str(pid_file)} if pid_file else {}),
                },
                stdin=asyncio.subprocess.PIPE,
                stdout=asyncio.subprocess.PIPE,
                stderr=errors,
                start_new_session=True,
                limit=2_000_000,
            )
            try:
                yield process
            finally:
                await disconnect(process)


async def disconnect(process):
    if process.returncode is None:
        try:
            os.killpg(process.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
    await process.wait()


async def send(process, message):
    process.stdin.write((json.dumps(message) + "\n").encode())
    await process.stdin.drain()


async def event(process, method):
    async with asyncio.timeout(30):
        while line := await process.stdout.readline():
            message = json.loads(line)
            if message.get("type") == "response" and message.get("success") is False:
                raise RuntimeError(message.get("error"))
            if method == "select" and message.get("method") == "notify" and message.get("message", "").startswith("ivy-probe "):
                raise RuntimeError(message["message"])
            if message.get("type") != "extension_ui_request" or message.get("method") != method:
                continue
            if method != "notify" or message.get("message", "").startswith("ivy-probe "):
                return message
    raise RuntimeError(f"Gentle exited before {method}")


async def question(process, command="/ivy-probe"):
    await send(process, {"type": "prompt", "message": command})
    return await event(process, "select")


async def reply(process, request, **answer):
    await send(process, {"type": "extension_ui_response", "id": request["id"], **answer})


async def expect_result(process, expected):
    result = await event(process, "notify")
    observed = json.loads(result["message"].removeprefix("ivy-probe "))
    if observed != expected:
        raise AssertionError(f"Expected {expected}, received {observed}")


async def main(installation):
    node = subprocess.check_output(["node", "-p", "process.execPath"], text=True).strip()
    shutil.copyfile(Path(__file__).with_suffix(".ts"), installation / "probe.ts")
    shutil.copyfile(Path(__file__).with_name("delegated.ts"), installation / "delegated.ts")
    async with AsyncExitStack() as stack:
        first = await stack.enter_async_context(session(installation, node))
        second = await stack.enter_async_context(session(installation, node))
        left, right = await asyncio.gather(question(first), question(second))
        print("Dialog fields:", sorted(left), flush=True)

        await reply(second, left, value="Left")
        await reply(second, right, value="Right")
        await expect_result(second, {"value": "Right"})
        await reply(second, right, value="Left")
        await reply(first, left, cancelled=True)
        await expect_result(first, {"cancelled": True})

        another = await question(second)
        await reply(second, right, value="Left")
        await reply(second, another, value="Right")
        await expect_result(second, {"value": "Right"})

        left, right = await asyncio.gather(
            question(first, "/ivy-delegate"), question(second, "/ivy-delegate")
        )
        await reply(second, left, value="Left")
        await reply(second, right, value="Right")
        await expect_result(second, {"value": "Right"})
        await reply(first, left, cancelled=True)
        await expect_result(first, {"cancelled": True})
        print("Verified reply and cancellation inside two delegated Pi processes.", flush=True)

        with TemporaryDirectory(prefix="ivy-gentle-witness-") as directory:
            pid_file = Path(directory) / "pending-child.pid"
            async with session(installation, node, pid_file) as interrupted:
                await question(interrupted, "/ivy-delegate")
                child_fd = os.pidfd_open(int(pid_file.read_text()))
                try:
                    watcher = select.poll()
                    watcher.register(child_fd, select.POLLIN)
                    await disconnect(interrupted)
                    if not await asyncio.to_thread(watcher.poll, 5000):
                        signal.pidfd_send_signal(child_fd, signal.SIGKILL)
                        raise AssertionError("Detached helper survived host disconnect")
                finally:
                    os.close(child_fd)

        pending = await question(first)
        await reply(first, pending, value="Left")
        # Break the connection before observing a result. Delivery is uncertain.
        await disconnect(first)
        replacement = await stack.enter_async_context(session(installation, node))
        fresh = await question(replacement)
        await reply(replacement, pending, value="Left")
        await reply(replacement, fresh, value="Right")
        await expect_result(replacement, {"value": "Right"})
        print("Passed session isolation, cancellation, duplicate and stale replies, and restart.")


if __name__ == "__main__":
    asyncio.run(main(Path(sys.argv[1]).resolve()))
