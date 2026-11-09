const ID = /^ses_[A-Za-z0-9]{1,124}$/;
const COMMANDS = new Set(["attach", "run", "serve", "web", "acp", "mini", "session", "db", "mcp"]);
const binary = (name) => /(?:^|[/\\])opencode(?:\.exe)?$/.test(name ?? "");

// This version's default TUI starts its own embedded worker. Other CLI modes
// are declined instead of treating a matching cwd or a loopback URL as proof.
export function supportedLocalInvocation(argv, env) {
  if (!Array.isArray(argv) || argv.length === 0) return false;
  let args;
  if (binary(argv[1]) || argv[1]?.startsWith("/$bunfs/")) args = argv.slice(2);
  else if (binary(argv[0])) args = argv.slice(1);
  else return false;
  for (const key of ["OPENCODE_EXPERIMENTAL", "OPENCODE_EXPERIMENTAL_WORKSPACES", "OPENCODE_WORKSPACE_ID"]) {
    if (env[key] && !["0", "false"].includes(env[key].toLowerCase())) return false;
  }
  let project = false;
  for (let i = 0; i < args.length; i++) {
    const arg = args[i];
    if (arg === "--session" || arg === "-s") {
      if (!ID.test(args[++i] ?? "")) return false;
    } else if (arg.startsWith("--session=")) {
      if (!ID.test(arg.slice(10))) return false;
    } else if (arg === "--print-logs") {
      continue;
    } else if (arg === "--log-level") {
      if (!["DEBUG", "INFO", "WARN", "ERROR"].includes(args[++i])) return false;
    } else if (!arg.startsWith("-") && !COMMANDS.has(arg) && !project) {
      project = true;
    } else return false;
  }
  return true;
}

export function routeSnapshot(api) {
  const empty = { session_id: null, cwd: null };
  if (api.app.version !== "1.18.31" || !api.state.ready) return empty;
  const route = api.route.current;
  if (route.name !== "session" || !ID.test(route.params?.sessionID ?? "")) return empty;
  const session = api.state.session.get(route.params.sessionID);
  // A selected child or a remote workspace is not an eligible root conversation.
  if (!session || session.parentID || session.workspaceID) return empty;
  const cwd = api.state.path.directory;
  if (typeof cwd !== "string" || !cwd.startsWith("/") || cwd.includes("\0")) return empty;
  return { session_id: route.params.sessionID, cwd };
}
