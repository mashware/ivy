import { createReporter, decodeReply, isAbsoluteHelper } from "../reporter.js";

export function register(on) {
  on("session.start", async ($, event, next) => {
    const paneDir = await $.env.get("PANEL_DIR");
    const paneId = await $.env.get("PANEL_ID");
    if (!paneDir || !paneId) return next(event);
    const helperPath = await $.env.get("IVY_SESSION_HELPER");
    if (!isAbsoluteHelper(helperPath)) return next(event);
    const version = await $.session.version();
    // Public APIs evolve; enable only the installed version verified for this adapter.
    if (version.version !== "2.1.319") return next(event);
    const surfaces = await $.session.surfaces();
    if (!surfaces.includes("terminal")) return next(event);
    const reporter = createReporter({
      client: "claude",
      invoke: async (argv, input) => decodeReply(await $.process.run(
        [helperPath, ...argv],
        { timeoutMs: 1000, ...(input === undefined ? {} : { stdin: JSON.stringify(input) }) },
      )),
      read: async () => ({ session_id: await $.session.id(), cwd: null }),
    });
    // A timer survives conversation changes and runs between turns without a prompt.
    // Claude cancels module-owned timers on unload/reload. Old generations then fail closed.
    await reporter.tick().catch(() => {});
    if (!reporter.isStopped()) $.clock.every(1000, () => reporter.tick());
    return next(event);
  });
}
