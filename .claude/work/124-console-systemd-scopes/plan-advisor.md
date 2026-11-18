# Delivery plan advisor

## Recommendation

Two ordered PRs are justified by the asymmetric-half test. The backend-only first PR provides a real mitigation: a console's OOM victim no longer shares the whole GUI/window cgroup, so other consoles survive even if the user still gets no specific ending notice. A UI-only first PR cannot truthfully identify systemd OOM and would be visibly incomplete. This split is worthwhile only if isolation can reach main before notice work is finished; otherwise merge the two into one PR and avoid an extra wave.

| n | wave | depends_on | includes | standalone | unlocks | risk alone on main | lines_est | files_est |
|---|---:|---|---|---|---|---|---:|---:|
| 1 | 1 | — | Linux-only gated Ivy re-exec launch path, parent-owned D-Bus `StartTransientUnit`, direct fallback, opening/resume/takeover integration, PTY and failure-path coverage. Do not add OOM wording. | Yes: complete per-console isolation with existing ending behavior. | #2 can add retained result classification and the notice. | If it remains alone, a killed console is isolated but the user still sees no confirmed reason. | ~650 | ~12 |
| 2 | 2 | #1 | Retain/query scope `Result`, distinguish `oom-kill` from unknown signals, connect to existing console notice, repaint/auto-close and state/UI coverage. | Yes on top of #1: adds truthful explanation; unknown stays unknown. | Closes the remaining user-facing behavior. | None beyond manager versions that cannot provide OOM evidence; those remain unknown by design. | ~300 | ~7 |

## Split tests and merge test

- Broken today: the reported incident kills unrelated consoles with the GUI. #1 repairs that mitigation gap immediately; bundling delays it behind the separate notice integration.
- Standalone ticket: none beyond ticket 124.
- Enabling slice: #1 is a dependency of only #2, so this does not qualify.
- Asymmetry: #1 makes the default branch safer; a UI-only slice cannot produce a reliable OOM notice without scope evidence.
- Merge test: considered #1 → #2, where #2 is the only dependent. Merging them loses the ability to ship the isolation mitigation while notice work continues; keep split only when that earlier mitigation has value. There are no same-wave pairs. No scaffolding-only PR is proposed: #1 actually changes process containment.

The backend/UI boundary is therefore viable, but it is not a technical necessity. If the team expects both slices to finish in one build/review window, combine them: launcher, evidence lifetime, and notice timing share one startup/exit contract, and one review avoids maintaining an intermediate no-notice state. No database, cross-client, or deployed-client migration applies.
