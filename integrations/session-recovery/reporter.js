// This module uses no client, file, model, shell, or environment APIs.
// Adapters provide the official live getter and an argv-only helper runner.
const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;
const OPEN_CODE_ID = /^ses_[A-Za-z0-9]+$/;
const OPAQUE = /^[A-Za-z0-9_-]{1,128}$/;

// Ivy supplies a verified absolute sibling path; never resolve a helper via PATH.
export function isAbsoluteHelper(value) {
  return typeof value === "string" && value.length > 1 && value.startsWith("/") &&
    !/[\u0000-\u001f\u007f-\u009f]/u.test(value) &&
    new TextEncoder().encode(value).length <= 4096;
}

export function decodeReply(result) {
  if (
    typeof result?.stdout !== "string" ||
    result.stdout.length > 4096 ||
    result.isStdoutTruncated
  ) throw new Error("invalid_helper_reply");
  let reply;
  try { reply = JSON.parse(result.stdout); }
  catch { throw new Error("invalid_helper_reply"); }
  if (typeof reply !== "object" || reply === null || typeof reply.ok !== "boolean") {
    throw new Error("invalid_helper_reply");
  }
  if (result.exitCode !== 0 && reply.ok) throw new Error("invalid_helper_reply");
  return reply;
}

function snapshot(client, value) {
  const id = value?.session_id;
  const valid = typeof id === "string" && id.length <= 128 &&
    (client === "claude" ? UUID.test(id) : OPEN_CODE_ID.test(id));
  if (!valid) return { session_id: null, cwd: null };
  const cwd = value.cwd;
  if (cwd !== null && (typeof cwd !== "string" || !cwd.startsWith("/") ||
    cwd.includes("\0") || new TextEncoder().encode(cwd).length > 4096)) {
    return { session_id: null, cwd: null };
  }
  return { session_id: id, cwd };
}

export function createReporter({ client, invoke, read }) {
  if (client !== "claude" && client !== "opencode") throw new Error("invalid_client");
  let generation = null;
  let sequence = 0;
  let published;
  let acknowledged = null;
  let stopped = false;
  let running = null;
  let changedWhileRunning = false;

  function forget() {
    generation = null;
    sequence = 0;
    published = undefined;
    acknowledged = null;
  }

  async function cycle() {
    // One immediate reattachment is enough; later timer ticks retry failures.
    for (let attempt = 0; attempt < 2 && !stopped; attempt++) {
      if (generation === null) {
        const attached = await invoke(["attach", client]);
        if (attached.error === "outside_ivy") stopped = true;
        if (stopped || !attached.ok) return;
        if (typeof attached.generation !== "string" || !OPAQUE.test(attached.generation)) {
          throw new Error("invalid_helper_reply");
        }
        generation = attached.generation;
      }
      const probed = await invoke(["probe", generation]);
      if (stopped) return;
      if (!probed.ok) {
        if (probed.error === "stale_generation") { forget(); continue; }
        return;
      }
      const challenge = probed.challenge ?? null;
      if (challenge !== null && (typeof challenge !== "string" || !OPAQUE.test(challenge))) {
        throw new Error("invalid_helper_reply");
      }
      // Read AFTER the probe: a close challenge must acknowledge a fresh read.
      const state = snapshot(client, await read());
      if (stopped) return;
      sequence++;
      if (!Number.isSafeInteger(sequence)) throw new Error("sample_sequence_exhausted");
      const key = JSON.stringify(state);
      if (key === published && (challenge === null || challenge === acknowledged)) return;
      const updated = await invoke(["update", generation], {
        sequence,
        session_id: state.session_id,
        cwd: state.cwd,
        challenge,
      });
      if (stopped) return;
      if (!updated.ok) {
        if (updated.error === "stale_generation") { forget(); continue; }
        return;
      }
      published = key;
      acknowledged = challenge;
      return;
    }
  }

  function request(urgent = false) {
    if (stopped) return Promise.resolve();
    if (running) {
      // Timer ticks never overlap. A route change may request one fresh follow-up.
      changedWhileRunning ||= urgent;
      return running;
    }
    running = (async () => {
      for (let passes = 0; passes < 2 && !stopped; passes++) {
        changedWhileRunning = false;
        await cycle();
        if (!changedWhileRunning) break;
      }
    })().finally(() => { running = null; });
    return running;
  }

  return {
    tick: () => request(),
    changed: () => request(true),
    stop: () => { stopped = true; },
    isStopped: () => stopped,
  };
}
