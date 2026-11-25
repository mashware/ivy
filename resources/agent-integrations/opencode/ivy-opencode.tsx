// OpenCode 1.18.31 TUI plugin. Only state metadata leaves this process.
import { createEffect, createRoot, createSignal } from "solid-js"
import { createConnection } from "node:net"
import { spawn, type ChildProcessWithoutNullStreams } from "node:child_process"
import { EventEmitter } from "node:events"
import { lstatSync } from "node:fs"

const envelope = (snapshot: Snapshot | null) => JSON.stringify({ tool: "agent_signal", v: 1, client: "opencode", snapshot }) + "\n"
type Snapshot = { session: string; activity: "busy" | "idle" | "unknown"; permission: boolean; question: boolean; stale?: boolean }
type Entry = { id: string; sessionID: string }

type Connection = {
  connecting: boolean
  write: (frame: string) => unknown
  end: () => unknown
  destroy: () => unknown
  on: (event: string, callback: (...args: any[]) => void) => unknown
}

// D115: inherited identity is not authentication; Windows delegates that boundary
// to the bundled native helper, never a PATH lookup or a repository executable.
function helperPath(value: string | undefined) {
  return !!value && /^[a-z]:[\\/]/i.test(value) && !/[\x00-\x1f\x7f-\x9f]/.test(value)
    && !value.slice(2).includes(":") && /\.exe$/i.test(value)
}

function validReply(reply: any, startup: boolean) {
  if (!reply || typeof reply !== "object" || Array.isArray(reply) || typeof reply.ok !== "boolean") return false
  const allowed = startup ? ["ok", "ready", "v"] : ["ok"]
  if (startup && (reply.ready !== true || reply.v !== 1)) return false
  if (!reply.ok) {
    allowed.push("error", "retryable")
    if (typeof reply.error !== "string") return false
    if (startup ? typeof reply.retryable !== "boolean" : reply.retryable !== undefined && typeof reply.retryable !== "boolean") return false
  }
  return Object.keys(reply).every(key => allowed.includes(key))
}

// D80: one in flight, one latest replacement; losing a connection is not an answer.
function channel(dir: string, connected: () => void) {
  let socket: Connection | undefined, child: ChildProcessWithoutNullStreams | undefined
  let retry: ReturnType<typeof setTimeout> | undefined, retryReply: ReturnType<typeof setTimeout> | undefined
  let deadlineTimer: ReturnType<typeof setTimeout> | undefined, waitingForRefresh = false
  let pending: Snapshot | null | undefined, inflightSnapshot: Snapshot | null | undefined
  let inflight = false, buffer = Buffer.alloc(0), stopped = false, disposing = false, ready = false, reporterReady = false
  let drained: (() => void) | undefined
  const windows = process.platform === "win32"
  const clearDeadline = () => { if (deadlineTimer) clearTimeout(deadlineTimer); deadlineTimer = undefined }
  function pump() {
    if (!socket || !ready || inflight || pending === undefined || waitingForRefresh) return
    inflight = true
    inflightSnapshot = pending
    pending = undefined
    if (inflightSnapshot === null) reporterReady = false
    if (windows) deadlineTimer = setTimeout(() => socket?.destroy(), 7000)
    const frame = envelope(inflightSnapshot)
    if (Buffer.byteLength(frame) - 1 > 65536) { socket.destroy(); return }
    socket.write(frame)
  }
  function connect() {
    if (stopped || disposing || socket || child) return
    let current: Connection
    if (windows) {
      const path = process.env.IVY_SESSION_HELPER
      if (!helperPath(path)) { stopped = true; return }
      let launched: ChildProcessWithoutNullStreams
      try {
        launched = spawn(path!, ["signals"], {shell: false, windowsHide: true, stdio: ["pipe", "pipe", "pipe"]})
      } catch { stopped = true; return }
      child = launched
      const stream = new EventEmitter() as EventEmitter & Connection
      stream.connecting = true
      stream.write = frame => launched.stdin.write(frame)
      stream.end = () => launched.stdin.end()
      stream.destroy = () => launched.kill()
      current = stream
      launched.stdout.on("data", chunk => stream.emit("data", chunk))
      launched.stdout.on("error", () => stream.destroy())
      launched.stdout.on("end", () => stream.destroy())
      launched.stderr.resume()
      launched.stdin.on("error", () => stream.destroy())
      launched.on("error", () => { stopped = true; stream.destroy() })
      launched.on("close", () => { if (child === launched) child = undefined; stream.emit("close") })
      deadlineTimer = setTimeout(() => stream.destroy(), 5000)
    } else {
      // A trailing slash or /. makes lstat follow a final symlink. Keep .. intact.
      let lookup = dir
      while (lookup.length > 1 && (lookup.endsWith("/") || lookup.endsWith("/."))) lookup = lookup.slice(0, -1)
      try {
        const stat = lstatSync(lookup)
        if (!stat.isDirectory() || stat.uid !== process.geteuid!() || (stat.mode & 0o077) !== 0) {
          retry = setTimeout(connect, 1000); return
        }
      } catch { retry = setTimeout(connect, 1000); return }
      current = createConnection(`${dir}/control.sock`)
    }
    socket = current
    ready = false; reporterReady = false
    current.on("connect", () => { if (socket === current) { ready = true; connected() } })
    current.on("data", (chunk: Buffer) => {
      if (socket !== current) return
      // Bound bytes before concatenating or parsing, including split UTF-8 chunks.
      let offset = 0
      while (offset < chunk.length) {
        const end = chunk.indexOf(10, offset)
        const stop = end < 0 ? chunk.length : end
        if (buffer.length + stop - offset > 65536) { current.destroy(); return }
        buffer = Buffer.concat([buffer, chunk.subarray(offset, stop)])
        if (end < 0) return
        const line = buffer; buffer = Buffer.alloc(0); offset = end + 1
        let reply: any
        try { reply = JSON.parse(new TextDecoder("utf-8", {fatal: true}).decode(line)) }
        catch { current.destroy(); return }
        if (!validReply(reply, !ready && windows)) { current.destroy(); return }
        if (!ready && windows) {
          clearDeadline()
          if (!reply.ok) { if (reply.retryable !== true) stopped = true; current.destroy(); return }
          ready = true; current.connecting = false; connected(); continue
        }
        if (!inflight || reply.ready !== undefined) { current.destroy(); return }
        clearDeadline()
        const sent = inflightSnapshot
        inflight = false; inflightSnapshot = undefined
        if (!reply.ok) {
          if (reply.retryable !== true) { stopped = true; current.destroy(); return }
          // Disposal's explicit null survives both an older refusal and its own
          // refusal. Refresh cannot run once the outer controller has stopped.
          const withdrawing = pending === null || sent === null
          pending = withdrawing ? null : undefined
          waitingForRefresh = true
          retryReply = setTimeout(() => {
            retryReply = undefined
            if (socket !== current || stopped) return
            waitingForRefresh = false
            if (pending === null) {
              pump()
              if (!disposing) connected()
            } else connected()
          }, 1000)
          continue
        }
        if (sent && !sent.stale) reporterReady = true
        pump()
        if (!inflight && pending === undefined) { drained?.(); drained = undefined }
      }
    })
    current.on("error", () => {})
    current.on("close", () => {
      if (socket !== current) return
      clearDeadline()
      if (retryReply) clearTimeout(retryReply)
      retryReply = undefined; waitingForRefresh = false
      socket = undefined; ready = false; pending = undefined; inflightSnapshot = undefined
      inflight = false; buffer = Buffer.alloc(0); reporterReady = false
      drained?.(); drained = undefined
      if (!stopped && !disposing) retry = setTimeout(connect, 1000)
    })
  }
  connect()
  return {
    beginDispose() { disposing = true; if (retry) clearTimeout(retry) },
    send(snapshot: Snapshot | null) {
      if ((disposing && snapshot !== null) || !socket || !ready || (waitingForRefresh && snapshot !== null) || (snapshot?.stale && !reporterReady)) return false
      if (snapshot === null && retryReply) { clearTimeout(retryReply); retryReply = undefined; waitingForRefresh = false }
      pending = snapshot
      if (snapshot === null) reporterReady = false
      pump()
      return true
    },
    async drain(deadline = Date.now() + 4000) {
      if (!socket || (!inflight && pending === undefined)) return
      await new Promise<void>(resolve => {
        const timeout = setTimeout(() => { drained = undefined; resolve() }, Math.max(0, Math.min(4000, deadline - Date.now())))
        drained = () => { clearTimeout(timeout); resolve() }
      })
    },
    async close(deadline = Date.now() + 750) {
      stopped = true
      if (retry) clearTimeout(retry)
      if (retryReply) clearTimeout(retryReply)
      clearDeadline()
      socket?.end()
      const ending = child
      if (!ending) { socket = undefined; return }
      const wait = async (limit: number) => {
        if (child !== ending) return
        await new Promise<void>(resolve => {
          const done = () => { clearTimeout(timeout); ending.off("close", done); resolve() }
          const timeout = setTimeout(done, Math.max(0, Math.min(limit, deadline - Date.now())))
          ending.once("close", done)
        })
      }
      await wait(250)
      if (child === ending) { ending.kill(); await wait(500) }
    },
  }
}

export default {
  id: "ivy.agent-signals",
  async tui(api: any) {
    const dir = process.env.PANEL_DIR
    if (!dir || !process.env.PANEL_ID) return
    let generation = 0, stopped = false, refreshQueued = false, refreshing = false, refreshPending = false
    let timer: ReturnType<typeof setTimeout> | undefined
    let latestIds: string[] = []
    const [children, setChildren] = createSignal<string[]>([])
    let requestRefresh = () => {}
    const transport = channel(dir, () => requestRefresh())
    let lastSnapshot: Snapshot | undefined, markedStale = false
    const markStale = () => {
      if (lastSnapshot && !markedStale) {
        markedStale = transport.send({...lastSnapshot, stale: true})
      }
    }
    const withdraw = () => { lastSnapshot = undefined; markedStale = false; transport.send(null) }
    const cancelTimer = () => { if (timer) clearTimeout(timer); timer = undefined }
    // Every invalidation makes any earlier asynchronous response ineligible.
    requestRefresh = () => {
      generation++
      if (stopped) return
      const route = api.route.current
      if (route.name !== "session") withdraw()
      else if (lastSnapshot && route.params?.sessionID !== lastSnapshot.session) markStale()
      refreshPending = true
      if (refreshQueued || refreshing) return
      refreshQueued = true
      queueMicrotask(runRefresh)
    }
    async function runRefresh() {
      refreshQueued = false
      if (stopped || refreshing || !refreshPending) return
      refreshing = true
      refreshPending = false
      try { await refresh(generation) }
      finally {
        refreshing = false
        if (!stopped && refreshPending && !refreshQueued) {
          refreshQueued = true
          queueMicrotask(runRefresh)
        }
      }
    }
    async function refresh(token: number) {
      cancelTimer()
      const route = api.route.current
      if (route.name !== "session" || typeof route.params?.sessionID !== "string") {
        if (latestIds.length) { latestIds = []; setChildren([]) }; withdraw(); return
      }
      const id = route.params.sessionID
      const session = api.state.session.get(id)
      if (!api.state.ready || !session) { markStale(); return }
      const directory = session.directory
      const workspace = session.workspaceID
      try {
        const [descendants, permissions, questions] = await Promise.all([
          session.parentID ? Promise.resolve({data: []}) : api.client.session.children({sessionID: id, directory, workspace}, {throwOnError: true}),
          api.client.permission.list({directory, workspace}, {throwOnError: true}),
          api.client.question.list({directory, workspace}, {throwOnError: true}),
        ])
        if (stopped || token !== generation || api.route.current.name !== "session" || api.route.current.params?.sessionID !== id) return
        if (!Array.isArray(descendants.data) || !Array.isArray(permissions.data) || !Array.isArray(questions.data)) { markStale(); return }
        const ids = session.parentID ? [id] : [id, ...descendants.data.filter((x: any) => x.parentID === id).map((x: any) => x.id)]
        if (ids.length !== latestIds.length || ids.some((x: string, i: number) => latestIds[i] !== x)) {
          latestIds = ids
          setChildren(ids)
        }
        const visiblePermissions: Entry[] = session.parentID ? [] : ids.flatMap((x: string) => api.state.session.permission(x))
        const visibleQuestions: Entry[] = session.parentID ? [] : ids.flatMap((x: string) => api.state.session.question(x))
        const pendingPermissions: Entry[] = permissions.data.filter((x: Entry) => ids.includes(x.sessionID))
        const pendingQuestions: Entry[] = questions.data.filter((x: Entry) => ids.includes(x.sessionID))
        // A missing getter entry has not been hydrated; never turn it into a clear.
        const missing = (pending: Entry[], visible: Entry[]) => pending.some(p => !visible.some(v => v.id === p.id && v.sessionID === p.sessionID))
        if (!session.parentID && (missing(pendingPermissions, visiblePermissions) || missing(pendingQuestions, visibleQuestions))) {
          markStale(); timer = setTimeout(requestRefresh, 2000); return
        }
        const exists = (visible: Entry[], pending: Entry[]) => visible.some(v => pending.some(p => p.id === v.id && p.sessionID === v.sessionID))
        const permission = exists(visiblePermissions, pendingPermissions)
        const question = exists(visibleQuestions, pendingQuestions)
        const status = api.state.session.status(id)?.type
        const activity = status === "busy" || status === "retry" ? "busy" : status === "idle" ? "idle" : "unknown"
        lastSnapshot = {session: id, activity, permission, question}
        markedStale = false
        transport.send(lastSnapshot)
        if (permission || question) timer = setTimeout(requestRefresh, 2000)
      } catch {
        // Keep the last claim; a failed refresh is not an answer.
        if (!stopped && token === generation) { markStale(); timer = setTimeout(requestRefresh, 2000) }
      }
    }
    const dispose = createRoot((dispose) => {
      createEffect(() => {
        const route = api.route.current
        const id = route.name === "session" ? route.params?.sessionID : undefined
        api.state.ready
        if (typeof id === "string") {
          api.state.session.get(id); api.state.session.status(id)
          for (const child of children()) { api.state.session.permission(child).length; api.state.session.question(child).length }
        }
        requestRefresh()
      })
      return dispose
    })
    const unsubscribe = ["session.created", "session.updated", "session.deleted", "session.status", "permission.asked", "permission.replied", "question.asked", "question.replied", "question.rejected"].map(
      type => api.event.on(type, () => requestRefresh()),
    )
    api.lifecycle.onDispose(async () => {
      const deadline = Date.now() + 4750
      stopped = true; generation++; cancelTimer(); dispose(); unsubscribe.forEach((off: () => void) => off())
      transport.beginDispose()
      withdraw()
      // Drain ordered frames within OpenCode’s five-second cleanup budget.
      await transport.drain(deadline - 750)
      await transport.close(deadline)
    })
  },
}
