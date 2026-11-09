import assert from "node:assert/strict";
import test from "node:test";
import { createReporter, decodeReply } from "../reporter.js";

const A = "11111111-1111-4111-8111-111111111111";
const B = "22222222-2222-4222-8222-222222222222";
const deferred = () => {
  let resolve;
  const promise = new Promise((done) => { resolve = done; });
  return { promise, resolve };
};

function fixture(client = "claude") {
  const calls = [];
  let id = client === "claude" ? A : "ses_Exact1";
  let challenge = null;
  let reads = 0;
  const reporter = createReporter({
    client,
    read: async () => { reads++; return { session_id: id, cwd: null }; },
    invoke: async (argv, input) => {
      calls.push({ argv, input });
      if (argv[0] === "attach") return { ok: true, generation: "root_1" };
      if (argv[0] === "probe") return { ok: true, challenge };
      return { ok: true };
    },
  });
  return { reporter, calls, setID: (value) => { id = value; },
    setChallenge: (value) => { challenge = value; }, reads: () => reads };
}

test("unchanged identity probes but does not republish", async () => {
  const f = fixture();
  await f.reporter.tick();
  await f.reporter.tick();
  assert.deepEqual(f.calls.map((call) => call.argv), [
    ["attach", "claude"], ["probe", "root_1"], ["update", "root_1"], ["probe", "root_1"],
  ]);
  assert.equal(f.reads(), 2);
});

test("new close challenge requires a fresh official read even for an unchanged ID", async () => {
  const f = fixture();
  await f.reporter.tick();
  f.setChallenge("close_1");
  await f.reporter.tick();
  assert.equal(f.reads(), 2);
  assert.deepEqual(f.calls.at(-1).input, { sequence: 2, session_id: A, cwd: null, challenge: "close_1" });
  await f.reporter.tick();
  assert.equal(f.calls.filter((call) => call.argv[0] === "update").length, 2);
});

test("read occurs after the probe has returned its challenge", async () => {
  let probed = false;
  const reporter = createReporter({
    client: "claude",
    invoke: async (argv, input) => {
      if (argv[0] === "attach") return { ok: true, generation: "g" };
      if (argv[0] === "probe") { probed = true; return { ok: true, challenge: "close" }; }
      assert.equal(input.session_id, B);
      assert.equal(input.challenge, "close");
      return { ok: true };
    },
    read: () => { assert(probed); return { session_id: B, cwd: null }; },
  });
  await reporter.tick();
});

test("stale generation discards the old sample and reads again after attaching", async () => {
  let generation = 0;
  let id = A;
  const updates = [];
  const reporter = createReporter({
    client: "claude",
    read: () => ({ session_id: id, cwd: null }),
    invoke: async (argv, input) => {
      if (argv[0] === "attach") return { ok: true, generation: `g${++generation}` };
      if (argv[0] === "probe") return { ok: true, challenge: null };
      updates.push({ argv, input });
      if (generation === 1) { id = B; return { ok: false, error: "stale_generation" }; }
      return { ok: true };
    },
  });
  await reporter.tick();
  assert.deepEqual(updates.map(({ argv, input }) => [argv[1], input.sequence, input.session_id]), [
    ["g1", 1, A], ["g2", 1, B],
  ]);
});

test("timer overlap is serialized and a route change gets a fresh follow-up", async () => {
  const pending = deferred();
  const entered = deferred();
  let id = "ses_One";
  const updates = [];
  const reporter = createReporter({
    client: "opencode",
    read: () => ({ session_id: id, cwd: "/work" }),
    invoke: async (argv, input) => {
      if (argv[0] === "attach") return { ok: true, generation: "g" };
      if (argv[0] === "probe") return { ok: true, challenge: null };
      updates.push(input);
      if (updates.length === 1) { entered.resolve(); await pending.promise; }
      return { ok: true };
    },
  });
  const first = reporter.tick();
  await entered.promise;
  assert.equal(reporter.tick(), first);
  id = "ses_Two";
  reporter.changed();
  assert.equal(updates.length, 1);
  pending.resolve();
  await first;
  assert.deepEqual(updates.map((value) => [value.sequence, value.session_id]), [[1, "ses_One"], [2, "ses_Two"]]);
});

test("stop during a delayed probe prevents later read and publication", async () => {
  const pending = deferred();
  const entered = deferred();
  let reads = 0;
  const reporter = createReporter({
    client: "claude",
    read: () => { reads++; return { session_id: A, cwd: null }; },
    invoke: async (argv) => {
      if (argv[0] === "attach") return { ok: true, generation: "g" };
      assert.equal(argv[0], "probe");
      entered.resolve();
      return pending.promise;
    },
  });
  const cycle = reporter.tick();
  await entered.promise;
  reporter.stop();
  pending.resolve({ ok: true, challenge: "close" });
  await cycle;
  await reporter.tick();
  assert.equal(reads, 0);
});

test("outside-Ivy response stops permanently, disabled response remains retryable", async () => {
  for (const [error, expected] of [["outside_ivy", 1], ["disabled", 2]]) {
    let calls = 0;
    const reporter = createReporter({ client: "claude", read: () => assert.fail(),
      invoke: async () => { calls++; return { ok: false, error }; } });
    await reporter.tick();
    await reporter.tick();
    assert.equal(calls, expected);
  }
});

test("invalid identities withdraw and extra client content never leaves the adapter", async () => {
  const sent = [];
  const reporter = createReporter({ client: "claude",
    read: () => ({ session_id: "--resume-last", cwd: "/work", transcript_path: "/private", prompt: "do not send" }),
    invoke: async (argv, input) => argv[0] === "attach" ? { ok: true, generation: "g" }
      : argv[0] === "probe" ? { ok: true, challenge: null } : (sent.push(input), { ok: true }),
  });
  await reporter.tick();
  assert.deepEqual(sent, [{ sequence: 1, session_id: null, cwd: null, challenge: null }]);
});

test("helper replies are bounded, typed, and consistent with their exit status", () => {
  assert.deepEqual(decodeReply({ exitCode: 1, stdout: '{"ok":false,"error":"stale_generation"}' }),
    { ok: false, error: "stale_generation" });
  for (const result of [
    { exitCode: 0, stdout: "x" }, { exitCode: 0, stdout: "{}" },
    { exitCode: 1, stdout: '{"ok":true}' }, { exitCode: 0, stdout: " ".repeat(4097) },
    { exitCode: 0, stdout: '{"ok":true}', isStdoutTruncated: true },
  ]) assert.throws(() => decodeReply(result), /invalid_helper_reply/);
});

test("missing generation cannot become the literal string undefined", async () => {
  const reporter = createReporter({ client: "claude", read: () => assert.fail(),
    invoke: async () => ({ ok: true }) });
  await assert.rejects(reporter.tick(), /invalid_helper_reply/);
});
