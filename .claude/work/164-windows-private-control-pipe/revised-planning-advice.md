# Revised planning advice: ticket 164

## Recommendation
Keep one atomic PR, but re-estimate it as L and plan review/validation accordingly. The Lifecycle revision adds essential Windows behavior: callback-free bounded services, committed snapshots, a nonblocking UI wake, release accounting and joined worker teardown. These are part of making the channel safe to use; they do not create a separate user-visible capability. The plan's prior ~1,600-line/~25-file estimate is stale. Use the design's ~2,400 added/modified lines across ~32 files as advisory planning signals, never budgets.

## Candidate-cut merge test

| Candidate cut | What a merge would deliver | Dependency and merge test | Outcome |
|---|---|---|---|
| A. Windows app service/lifecycle foundation: snapshots, HWND wake, release slots, UI drain and shutdown ownership | Infrastructure with no working Windows control channel | B must consume these exact contracts. A alone advertises no usable feature, and offers no independent current behavior. | Reject as a separate PR; it adds a wave and leaves substantial unexercised plumbing. |
| B. Secure transport and identity: private directory, PID propagation/authentication, native overlapped pipe, discovery and tests | Complete Windows channel, including lifecycle contract | B depends on A if split. If B is merged first, enabled discovery could reach unsafe/incomplete teardown; if A is merged first, no capability is enabled. Keep both in one reviewed merge so capability, guards and proof arrive together. | Combine A+B as PR #1. |

Other tempting cuts (directory/PID forwarding, transport facade, or discovery) also fail the standalone-value test: each is either unused setup or an incomplete advertised endpoint. No asymmetric safety gain justifies a preparatory merge. There is one wave: `#1 (depends_on: [])`; no later wave.

## Risks and estimates
The main risk is integration breadth: Windows-only service constructors and app publishers cross panel-control, panel-contract, app UI/document/plan code, launch environment forwarders, MCP discovery and native CI. The most delicate review areas are HWND lifetime and paint-to-drain behavior, release-slot reuse/order under a stopped UI, and cancellation/completion races during concurrent shutdown. Native Windows proof remains required for ACL/access checks, remote rejection, UI wake and zero-worker drop; Unix checks alone do not establish these properties. Preserve the honest manual-proof status if intended-pane rendering cannot be observed.

Advisory estimate for the combined PR: ~2,400 added/modified lines, ~32 files, per revised design; allow variance after implementation inventory. No per-PR estimates apply because the candidate split is rejected. Estimates guide review capacity only.
