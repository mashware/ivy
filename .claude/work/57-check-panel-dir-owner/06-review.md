# Code review 57

## Summary
- Reviewed: main (3d2dedc)...ec654cd, frozen first review; no untracked source files.
- Review tier: proportional, S + authorization control-flow change: xhigh effort; independent security, correctness and blinded contract audits.
- Review path: flow bundle material; agentic reviewers because agents.exec_cmd is empty; built-in Claude code-review unavailable in Codex, parent performed its correctness/reuse pass and two independent reviewers covered security and correctness.
- Config read: FLOW.codex.md — FLOW_CONTEXT=unattended: FLOW.unattended.md not found.
- Agent models: models.agents=gpt-6-luna (FLOW.codex.md).
- Cost: 3/12 subagents launched (2 reviewers · 1 contract auditor), tier proportional, effort xhigh.
- Measured cost: not reported.
- Critical findings: 0. Suggestions: 0. No fixes applied after frozen reading.

## CLI tier (verbatim)
```text
Changed lines: 330 over 9 file(s), `3d2dedc04fe8`..`ec654cdfa42c` (base `main`)
Config read: FLOW.codex.md — FLOW_CONTEXT=unattended: FLOW.unattended.md not found
review_depth: proportional
Diff size: S   ·   meta.json.size: M   ·   Effective size: S
Tier: proportional → built-in at `high`, panel: only on a sensitive surface
With the sensitive-surface bump: built-in at `xhigh`, panel always
Budget: budget_max 12 · fanout_max 4
Within 10% under a threshold: no
quality.sensitive_paths matched: none (the key is empty)

Left to you, because a command cannot see it: whether the generic categories apply
(auth, secrets, payments, personal data, a public contract, a schema change), and
whether a matched path is a control-flow change or observability only — a log level,
a metric, a message text gets the review its size already earned. Decide, then record
which of the two it was. Any deviation from the tier above goes in 06-review.md with
its reason.

```

## Coverage
All 9 changed files examined. Security reviewer checked inherited caller boundaries, metadata predicate and JS retries; correctness reviewer checked lifecycle, platform cfg and tests; parent checked docs, helper response shapes and YAGNI.
Completeness sweep and adversarial round skipped: effective size S, zero ambiguous findings.
No targeted reinforcement roles configured. No new architectural pieces: idiom audit not applicable.

## Double-blind contract verification
All 7 contract groups match: MCP error envelope, helper envelope/exit/stderr, outside_ivy, privacy text, gone text, other metadata errors, plugin retry/disposal. No mismatches.

## Over-engineering
Only new defensive mechanism: per-attempt private-directory check, justified by planted stale console path. Reuses reconnect timer. Nothing to trim.

## Data-access duel
No queries or schema changes.

## Blockers
None.

## Quality gates
- cargo fmt --all --check: passed.
- cargo clippy --locked --workspace --all-targets -- -D warnings: passed.
- Focused Rust, simulated and real socket plugin tests, Node 24 session-recovery: passed.

## Next step
Validate full workspace and coverage.

## Validation test-only delta
Reviewed revision ec654cdfa42c; validation revision bd8493f. Only crates/panel-control/tests/inherited_directory.rs changed: added genuine inherited bridge success through a live channel and a local spelling variable. Parent inspected; ship's test-only delta rule applies. No production change since review.

## Validation round 1 repair review
Frozen delta ec654cd...ca90853: tests only, two files. Independent reviewer confirmed genuine bridge exchange assertions and the new script-audit registry row; no skipped, deleted or loosened tests. Initial whole-suite run found MODULES missing the new test file; registry now includes it. Focused script audit: 5/5 passed. Parent also inspected the literal four-line row. Review cost for this delta: 1 agent; no findings. Production code is unchanged from ec654cd.
