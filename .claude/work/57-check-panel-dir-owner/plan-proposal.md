# Delivery plan proposal — #57

Recommend **one PR, one wave**. The design defines a single inherited-directory trust boundary implemented by the Rust clients and direct OpenCode plugin, with corresponding diagnostics and security documentation. No slice qualifies for independent delivery: there is one ticket, neither language alone fixes all inherited connection paths, and docs alone do not repair the defect.

| n | wave | depends_on | includes | standalone | risk | lines_est | files_est |
|---|---:|---|---|---|---|---:|---:|
| 1 | 1 | — | Shared Rust private-directory checker and unit coverage; bridge and session-helper checks before connect with preserved refusal detail and caller-level tests; direct OpenCode lstat check before each attempt with refusal/retry/disposal fixture coverage; D7, architecture security text, and endpoint explanation updates | Yes — complete #57 end to end | Medium: filesystem spelling/metadata rules and lazy/retry connection paths span Rust and JS; focused fixtures and one review keep the shared invariant aligned | 250–450 | 9–13 |

**Rust + JS:** Keep together. Splitting gives a smaller language-specific review, but each PR would leave one inherited connection path accepting the planted directory. It also creates ordering/dependency overhead without enabling parallel implementation: both must honor the same owner, type, mode, symlink-spelling, and retry rules. Combining does broaden review across two test stacks; the design’s explicit contracts make that cost bounded.

**Docs + code:** Keep together. The current endpoint explanation says no additional access control is needed, while D7 and architecture describe the privacy boundary. A code-only merge leaves materially false security guidance; a docs-only PR has no standalone defect repair and could describe behavior that has not shipped. Updating them with the guard makes the contract reviewable against implementation.

Estimates are rough scope indicators, not targets. No prerequisite ticket or later wave is needed. This is the smallest complete delivery; splitting would delay full protection without independent user value.
