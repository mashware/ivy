// OpenCode 1.18.31 TUI plugin. Only state metadata leaves this process.
import { createEffect, createRoot, createSignal } from "solid-js"
import { createConnection, type Socket } from "node:net"
import { join } from "node:path"

const envelope = (snapshot: Snapshot | null) => JSON.stringify({ tool: "agent_signal", v: 1, client: "opencode", snapshot }) + "\n"
type Snapshot = { session: string; activity: "busy" | "idle" | "unknown"; permission: boolean; question: boolean; stale?: boolean }
type Entry = { id: string; sessionID: string }

// One request in flight and one latest replacement. Read each response before
// writing the next one; neither a slow endpoint nor an outage accumulates frames.
function channel(path: string, connected: () => void) {
  let socket: Socket | undefined, retry: ReturnType<typeof setTimeout> | undefined
  let pending: string | undefined, inflight = false, buffer = "", stopped = false
  let drained: (() => void) | undefined
  function pump() {
    if (!socket || socket.connecting || inflight || pending === undefined) return
    inflight = true
    const line = pending
    pending = undefined
    socket.write(line)
  }
  function connect() {
    if (stopped) return
    const current = createConnection(path)
    socket = current
    current.on("connect", () => { if (socket === current) connected() })
    current.on("data", (chunk) => {
      if (socket !== current) return
      buffer += chunk.toString()
      if (buffer.length > 65536) { current.destroy(); return }
      const end = buffer.indexOf("\n")
      if (end < 0) return
      const line = buffer.slice(0, end)
      buffer = buffer.slice(end + 1)
      try { if (JSON.parse(line).ok !== true) { current.destroy(); return } }
      catch { current.destroy(); return }
      inflight = false
      pump()
      if (!inflight && pending === undefined) { drained?.(); drained = undefined }
    })
    current.on("error", () => {})
    current.on("close", () => {
      if (socket !== current) return
      socket = undefined; pending = undefined; inflight = false; buffer = ""
      drained?.(); drained = undefined
      if (!stopped) retry = setTimeout(connect, 1000)
    })
  }
  connect()
  return {
    send(snapshot: Snapshot | null) { if (!socket || socket.connecting) return; pending = envelope(snapshot); pump() },
    async drain() {
      if (!socket || (!inflight && pending === undefined)) return
      await new Promise<void>(resolve => {
        const timeout = setTimeout(() => { drained = undefined; resolve() }, 4000)
        drained = () => { clearTimeout(timeout); resolve() }
      })
    },
    close() { stopped = true; if (retry) clearTimeout(retry); socket?.end(); socket = undefined },
  }
}

export default {
  id: "ivy.agent-signals",
  async tui(api: any) {
    const dir = process.env.PANEL_DIR
    if (!dir || !process.env.PANEL_ID || process.platform === "win32") return
    let generation = 0, stopped = false, refreshQueued = false, refreshing = false, refreshPending = false
    let timer: ReturnType<typeof setTimeout> | undefined
    let latestIds: string[] = []
    const [children, setChildren] = createSignal<string[]>([])
    let requestRefresh = () => {}
    const transport = channel(join(dir, "control.sock"), () => requestRefresh())
    let lastSnapshot: Snapshot | undefined, markedStale = false
    const markStale = () => {
      if (lastSnapshot && !markedStale) {
        markedStale = true
        transport.send({...lastSnapshot, stale: true})
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
      stopped = true; generation++; cancelTimer(); dispose(); unsubscribe.forEach((off: () => void) => off())
      withdraw()
      // Drain ordered frames within OpenCode’s five-second cleanup budget.
      await transport.drain()
      transport.close()
    })
  },
}
