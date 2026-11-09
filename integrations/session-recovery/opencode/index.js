import { createEffect, createRoot } from "solid-js";
import { createReporter, isAbsoluteHelper } from "../reporter.js";
import { invokeHelper } from "./helper.js";
import { routeSnapshot, supportedLocalInvocation } from "./state.js";

export default {
  id: "mashware.ivy-session-recovery",
  async tui(api) {
    const helperPath = process.env.IVY_SESSION_HELPER;
    if (!process.env.PANEL_DIR || !process.env.PANEL_ID || !isAbsoluteHelper(helperPath) ||
        api.app.version !== "1.18.31" ||
        !supportedLocalInvocation(process.argv, process.env)) return;
    const reporter = createReporter({
      client: "opencode",
      invoke: (argv, input) => invokeHelper(helperPath, argv, input),
      read: () => routeSnapshot(api),
    });
    // The official loader shares the host's Solid runtime with TUI plugins.
    let dispose;
    createRoot((release) => {
      dispose = release;
      createEffect(() => {
        routeSnapshot(api); // Subscribe to route, root metadata and local path changes.
        void reporter.changed().catch(() => {});
      });
    });
    const timer = setInterval(() => { void reporter.tick().catch(() => {}); }, 1000);
    api.lifecycle.onDispose(() => {
      clearInterval(timer);
      reporter.stop();
      dispose();
    });
  },
};
