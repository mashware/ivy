import { spawn } from "node:child_process";
import { writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { answerThroughUi } from "./node_modules/gentle-pi/extensions/gentle-agents.ts";
import { AgentRunner } from "./node_modules/gentle-pi/lib/agents-runner.ts";
import { TaskStore } from "./node_modules/gentle-pi/lib/agents-protocol.ts";

const fixture = fileURLToPath(import.meta.url);

export default function register(pi) {
  if (process.env.GENTLE_PI_AGENTS_CHILD === "1") {
    // The runner starts a detached process group. End the fixture if its parent disappears.
    process.once("disconnect", () => process.exit(0));
    if (!process.connected) process.exit(1);
    pi.registerCommand("ivy-child-dialog", {
      description: "Ask from inside the delegated Pi process",
      handler: async (_, ctx) => {
        if (process.env.IVY_PROBE_PID_FILE) {
          writeFileSync(process.env.IVY_PROBE_PID_FILE, String(process.pid));
        }
        const value = await ctx.ui.select("Choose a route", ["Left", "Right"]);
        // Only the child that resumed can emit this experiment receipt.
        process.send?.({ ivyProbeReceipt: value === undefined ? { cancelled: true } : { value } });
      },
    });
    return;
  }

  pi.registerCommand("ivy-delegate", {
    description: "Run the deterministic task-mode helper",
    handler: async (_, ctx) => {
      const receipt = Promise.withResolvers();
      const completion = Promise.withResolvers();
      const runner = new AgentRunner(new TaskStore(), { maxConcurrency: 1, stallTimeoutMs: 20000 }, {
        pi: { command: process.execPath, args: [join(dirname(fixture),
          "node_modules/@earendil-works/pi-coding-agent/dist/bundle/cli.js")] },
        now: Date.now,
        schedule(fn, ms) {
          const timer = setTimeout(fn, ms);
          return () => clearTimeout(timer);
        },
        spawn(command, args, options) {
          const child = spawn(command, args, options);
          child.on("message", message => {
            if (message && typeof message === "object" && "ivyProbeReceipt" in message) {
              receipt.resolve(message.ivyProbeReceipt);
            }
          });
          return child;
        },
      }, {
        askUser: (_taskId, ask, raw) => answerThroughUi(ctx.ui, ask, raw),
        onFinish: task => {
          completion.resolve();
          receipt.reject(new Error(task.error ?? task.status));
        },
      });
      const task = runner.run({
        agent: { name: "ivy-probe", instructions: "", tools: [], scope: "global" },
        mode: "task", prompt: "/ivy-child-dialog", parentSessionId: "ivy-probe",
        cwd: process.cwd(), sessionDir: process.cwd(), env: process.env,
        noExtensions: true, extensionPaths: [fixture],
      });
      let result;
      try {
        result = await receipt.promise;
      } finally {
        runner.cancel(task.id);
        await completion.promise;
      }
      ctx.ui.notify("ivy-probe " + JSON.stringify(result));
    },
  });
}
