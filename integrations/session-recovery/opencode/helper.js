import { spawn } from "node:child_process";
import { decodeReply, isAbsoluteHelper } from "../reporter.js";

export function invokeHelper(helperPath, argv, input) {
  if (!isAbsoluteHelper(helperPath)) return Promise.reject(new Error("unsupported_helper_path"));
  return new Promise((resolve, reject) => {
    const child = spawn(helperPath, argv, {
      shell: false,
      windowsHide: true,
      stdio: ["pipe", "pipe", "ignore"],
    });
    let output = "";
    let oversized = false;
    const timer = setTimeout(() => child.kill("SIGKILL"), 1000);
    child.stdout.setEncoding("utf8");
    child.stdout.on("data", (chunk) => {
      output += chunk;
      if (output.length > 4096) { oversized = true; child.kill("SIGKILL"); }
    });
    child.once("error", () => { clearTimeout(timer); reject(new Error("helper_unavailable")); });
    child.once("close", (exitCode) => {
      clearTimeout(timer);
      try { resolve(decodeReply({ exitCode, stdout: output, isStdoutTruncated: oversized })); }
      catch (error) { reject(error); }
    });
    child.stdin.on("error", () => {});
    child.stdin.end(input === undefined ? "" : JSON.stringify(input));
  });
}
