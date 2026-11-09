import assert from "node:assert/strict";
import { readFile, mkdtemp, writeFile, chmod, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, relative } from "node:path";
import { spawn } from "node:child_process";
import test from "node:test";
import { register } from "../hooks/register.js";
import { routeSnapshot, supportedLocalInvocation } from "../opencode/state.js";
import { invokeHelper } from "../opencode/helper.js";
import { isAbsoluteHelper } from "../reporter.js";

const UUID = "11111111-1111-4111-8111-111111111111";
const HELPER = "/opt/Ivy tools/ivy-session";

test("Claude registers only a metadata timer, keeps next(event), and uses exact argv/stdin", async () => {
  let handler;
  register((event, fn) => { assert.equal(event, "session.start"); handler = fn; });
  const calls = [], envReads = [], timers = [];
  const $ = {
    env: { get: async (key) => { envReads.push(key); return key === "IVY_SESSION_HELPER" ? HELPER : "fixture"; } },
    session: { version: async () => ({ version: "2.1.319" }),
      surfaces: async () => ["terminal"], id: async () => UUID },
    clock: { every: (ms, fn) => { timers.push({ ms, fn }); return { cancel() {} }; } },
    process: { run: async (argv, init) => {
      calls.push({ argv, init });
      const reply = argv[1] === "attach" ? { ok: true, generation: "g" }
        : argv[1] === "probe" ? { ok: true, challenge: null } : { ok: true };
      return { exitCode: 0, stdout: JSON.stringify(reply) };
    } },
  };
  const event = Object.freeze({ cwd: "/work" });
  assert.equal(await handler($, event, (same) => same), event);
  assert.deepEqual(envReads, ["PANEL_DIR", "PANEL_ID", "IVY_SESSION_HELPER"]);
  assert.equal(timers.length, 1);
  assert.equal(timers[0].ms, 1000);
  assert.deepEqual(calls.map((call) => call.argv), [
    [HELPER, "attach", "claude"], [HELPER, "probe", "g"], [HELPER, "update", "g"],
  ]);
  assert.deepEqual(JSON.parse(calls[2].init.stdin), { sequence: 1, session_id: UUID, cwd: null, challenge: null });
  await timers[0].fn();
  assert.equal(calls.length, 4); // Unchanged ID: only a probe, no publication.
});

test("Claude installed globally does nothing outside Ivy", async () => {
  let handler;
  register((_, fn) => { handler = fn; });
  const $ = { env: { get: async () => undefined }, process: { run: () => assert.fail() },
    clock: { every: () => assert.fail() }, session: { version: () => assert.fail() } };
  assert.equal(await handler($, "event", (event) => event), "event");
});

test("Claude headless/SDK surface cannot attach even with inherited pane variables", async () => {
  let handler;
  register((_, fn) => { handler = fn; });
  const $ = { env: { get: async (key) => key === "IVY_SESSION_HELPER" ? HELPER : "fixture" }, process: { run: () => assert.fail() },
    clock: { every: () => assert.fail() }, session: { version: async () => ({ version: "2.1.319" }),
      surfaces: async () => [] } };
  assert.equal(await handler($, "event", (event) => event), "event");
});

test("missing, relative or control-bearing helper locator never starts Claude work", async () => {
  for (const helper of [undefined, "", "ivy-session", "./ivy-session", "/", "/tmp/helper\n", "/tmp/helper\u0000", "/tmp/helper\u009f"]) {
    assert.equal(isAbsoluteHelper(helper), false);
    let handler;
    register((_, fn) => { handler = fn; });
    const $ = { env: { get: async (key) => key === "IVY_SESSION_HELPER" ? helper : "fixture" },
      process: { run: () => assert.fail() }, clock: { every: () => assert.fail() },
      session: { version: () => assert.fail() } };
    assert.equal(await handler($, "event", (event) => event), "event");
    await assert.rejects(invokeHelper(helper, ["attach", "opencode"]), /unsupported_helper_path/);
  }
  assert.equal(isAbsoluteHelper(HELPER), true);
});

test("OpenCode accepts native/Bun direct restore, declines unsupported topology", () => {
  for (const argv of [["/usr/bin/opencode"], ["bun", "/$bunfs/root/src/index.js"],
    ["/usr/bin/opencode", "--session", "ses_Exact1"],
    ["bun", "/$bunfs/root/src/index.js", "--session", "ses_Exact1"]]) {
    assert(supportedLocalInvocation(argv, {}));
  }
  for (const args of [["attach", "http://localhost:4096"], ["serve"], ["run"],
    ["--port", "4096"], ["--mini"], ["--prompt", "text"], ["--session", "--continue"]]) {
    assert(!supportedLocalInvocation(["/usr/bin/opencode", ...args], {}));
  }
  for (const key of ["OPENCODE_EXPERIMENTAL", "OPENCODE_EXPERIMENTAL_WORKSPACES", "OPENCODE_WORKSPACE_ID"]) {
    assert(!supportedLocalInvocation(["/usr/bin/opencode"], { [key]: "enabled" }));
  }
});

test("OpenCode reads route metadata only and withdraws children/workspaces/home", () => {
  let metadata = {};
  const api = { app: { version: "1.18.31" }, route: { current: { name: "session", params: { sessionID: "ses_Exact1" } } },
    state: { ready: true, path: { directory: "/work" }, session: {
      get: () => new Proxy(metadata, { get: (value, key) => {
        assert(["parentID", "workspaceID"].includes(key)); return value[key];
      } }),
      messages: () => assert.fail("must not inspect messages"),
    } } };
  assert.deepEqual(routeSnapshot(api), { session_id: "ses_Exact1", cwd: "/work" });
  for (const value of [{ parentID: "ses_Parent" }, { workspaceID: "remote" }]) {
    metadata = value;
    assert.deepEqual(routeSnapshot(api), { session_id: null, cwd: null });
  }
  metadata = {};
  for (const name of ["home", "custom"]) {
    api.route.current = { name };
    assert.equal(routeSnapshot(api).session_id, null);
  }
});

test("actual child runner passes literal argv/stdin and captures helper output", async () => {
  const directory = await mkdtemp(join(tmpdir(), "ivy-helper-fixture-"));
  const oldPath = process.env.PATH;
  const oldTrace = process.env.IVY_HELPER_FIXTURE_TRACE;
  const trace = join(directory, "trace.json");
  try {
    const helper = join(directory, "trusted helper $(not-executed) 'quoted'");
    await writeFile(helper, '#!/usr/bin/python3\nimport json,os,sys\nopen(os.environ["IVY_HELPER_FIXTURE_TRACE"],"w").write(json.dumps({"argv":sys.argv[1:],"input":sys.stdin.read()}))\nprint(json.dumps({"ok":True}))\n');
    await chmod(helper, 0o700);
    await writeFile(join(directory, "ivy-session"), '#!/bin/sh\nexit 78\n');
    await chmod(join(directory, "ivy-session"), 0o700);
    process.env.PATH = relative(process.cwd(), directory);
    process.env.IVY_HELPER_FIXTURE_TRACE = trace;
    const input = { sequence: 3, session_id: "ses_Exact1", cwd: "/work", challenge: "close" };
    assert.deepEqual(await invokeHelper(helper, ["update", "literal argument with spaces"], input), { ok: true });
    assert.deepEqual(JSON.parse(await readFile(trace, "utf8")), {
      argv: ["update", "literal argument with spaces"], input: JSON.stringify(input),
    });
  } finally {
    process.env.PATH = oldPath;
    if (oldTrace === undefined) delete process.env.IVY_HELPER_FIXTURE_TRACE;
    else process.env.IVY_HELPER_FIXTURE_TRACE = oldTrace;
    await rm(directory, { recursive: true, force: true });
  }
});

test("Codex static guarded command uses the absolute locator literally and never PATH", async () => {
  const config = JSON.parse(await readFile(new URL("../codex/hooks.json", import.meta.url), "utf8"));
  const command = config.hooks.SessionStart[0].hooks[0].command;
  assert.equal(command, 'if [ -n "$PANEL_DIR" ] && [ -n "$PANEL_ID" ] && [ -n "$IVY_SESSION_HELPER" ]; then case "$IVY_SESSION_HELPER" in /*) case "$IVY_SESSION_HELPER" in *[[:cntrl:]]*) ;; *) "$IVY_SESSION_HELPER" hook codex ;; esac ;; esac; fi');
  const directory = await mkdtemp(join(tmpdir(), "ivy-codex-hook-"));
  const trace = join(directory, "trace.json");
  const helper = join(directory, "trusted helper $(not-executed) 'quoted'");
  try {
    await writeFile(helper, '#!/usr/bin/python3\nimport json,os,sys\nopen(os.environ["IVY_HELPER_FIXTURE_TRACE"],"w").write(json.dumps({"argv":sys.argv[1:],"input":sys.stdin.read()}))\n');
    await chmod(helper, 0o700);
    await writeFile(join(directory, "ivy-session"), '#!/bin/sh\nexit 78\n');
    await chmod(join(directory, "ivy-session"), 0o700);
    const run = (changes) => new Promise((resolve, reject) => {
      const child = spawn("/bin/sh", ["-c", command], { cwd: directory, stdio: ["pipe", "ignore", "ignore"],
        env: { PATH: ".", PANEL_DIR: "/pane", PANEL_ID: "pane", IVY_SESSION_HELPER: helper,
          IVY_HELPER_FIXTURE_TRACE: trace, ...changes } });
      child.once("error", reject);
      child.once("close", (code) => code === 0 ? resolve() : reject(new Error(`hook exit ${code}`)));
      child.stdin.on("error", (error) => { if (error.code !== "EPIPE") reject(error); });
      child.stdin.end('{"official":"metadata"}');
    });
    for (const changes of [{ PANEL_DIR: "" }, { PANEL_ID: "" }, { IVY_SESSION_HELPER: "" },
      { IVY_SESSION_HELPER: "ivy-session" }, { IVY_SESSION_HELPER: "/tmp/helper\n" }]) {
      await run(changes);
      await assert.rejects(readFile(trace), { code: "ENOENT" });
    }
    await run({});
    assert.deepEqual(JSON.parse(await readFile(trace, "utf8")), {
      argv: ["hook", "codex"], input: '{"official":"metadata"}',
    });
  } finally { await rm(directory, { recursive: true, force: true }); }
});
