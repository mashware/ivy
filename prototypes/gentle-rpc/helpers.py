"""Verify Gentle's owner-scoped helper lists and pushed updates using two installed launchers."""

import argparse
import asyncio
from contextlib import AsyncExitStack
import json
from pathlib import Path
import subprocess
from tempfile import TemporaryDirectory

from stop import AGENT, SCHEMA, Model, session


class Updates:
    def __init__(self, stdout):
        self.stdout = stdout
        self.receipts = []

    async def readline(self):
        line = await self.stdout.readline()
        if line:
            record = json.loads(line)
            if (record.get("type"), record.get("method"), record.get("widgetKey")) == (
                "extension_ui_request", "setWidget", "gentle-desktop-rpc"
            ):
                for item in record.get("widgetLines", []):
                    envelope = json.loads(item)
                    if (envelope.get("schema"), envelope.get("kind"), envelope.get("event")) == (
                        SCHEMA, "event", "agents_update"
                    ):
                        seq = envelope["seq"]
                        assert not self.receipts or self.receipts[-1][0] < seq
                        self.receipts.append((seq, [
                            (row["id"], row["agent"], row["status"]) for row in envelope["data"]["agents"]
                        ]))
        return line

    def saw(self, helper_id, state):
        return any((helper_id, AGENT, state) in rows for _, rows in self.receipts)


async def main(installation):
    node = subprocess.check_output(["node", "-p", "process.execPath"], text=True).strip()
    launcher = installation / "node_modules/gentle-pi/bin/gentle-shell.mjs"
    model = Model(False)
    with TemporaryDirectory(prefix="ivy-gentle-helpers-") as directory:
        async with AsyncExitStack() as stack:
            server = await stack.enter_async_context(await asyncio.start_server(model.serve, "127.0.0.1", 0))
            port = server.sockets[0].getsockname()[1]
            a, b = [await stack.enter_async_context(session(node, launcher, Path(directory) / name, port)) for name in "AB"]
            stack.callback(model.release_b.set)
            for host in (a, b):
                host.process.stdout = Updates(host.process.stdout)
                commands = (await host.native("get_commands"))["commands"]
                assert any(c["name"] == "gentle:desktop" and c["source"] == "extension" for c in commands)
                capabilities = (await host.call("capabilities"))["data"]
                assert "list_subagents" in capabilities["methods"] and "agents_update" in capabilities["events"]
                assert await host.agents() == []
            print("Both native owners advertised list_subagents and agents_update and returned empty lists.", flush=True)
            await asyncio.gather(a.prompt("parent A"), b.prompt("parent B"))
            async with asyncio.timeout(60):
                while True:
                    lists = await asyncio.gather(a.agents(), b.agents())
                    if all(len(rows) == 1 and rows[0]["status"] == "running" for rows in lists) and all(
                        model.started[name].is_set() for name in "AB"
                    ):
                        break
                    await asyncio.sleep(0.1)
            ida, idb = (rows[0]["id"] for rows in lists)
            assert ida != idb and all(rows[0]["agent"] == AGENT for rows in lists)
            assert (await a.call("cancel_subagent", id=ida))["ok"]
            async with asyncio.timeout(20):
                while True:
                    [ra], [rb] = await asyncio.gather(a.agents(), b.agents())
                    assert (rb["id"], rb["status"]) == (idb, "running")
                    if ra["status"] == "cancelled" and a.process.stdout.saw(ida, "cancelled"):
                        break
                    await asyncio.sleep(0.1)
            model.release_b.set()
            async with asyncio.timeout(20):
                while not b.process.stdout.saw(idb, "completed"):
                    await b.agents()
                    await asyncio.sleep(0.1)
            for label, host, expected, final in [("A", a, ida, "cancelled"), ("B", b, idb, "completed")]:
                seen = host.process.stdout
                assert seen.saw(expected, "running") and seen.saw(expected, final)
                assert all(row[0] == expected for _, rows in seen.receipts for row in rows)
                print(f"{label} {expected}: " + ", ".join(f"{seq}={rows[0][2]}" for seq, rows in seen.receipts if rows))
            print("B remained running while A cancelled. Each pipe reported only its own helper ID.")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("installation", type=Path)
    asyncio.run(main(parser.parse_args().installation.resolve()))
