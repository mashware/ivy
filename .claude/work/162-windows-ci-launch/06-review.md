# Review 162

## Summary
- Review path: pinned flow-workflows 0.118.0 bundle --checks, frozen first candidate a0952cfcc9a3 against origin/main (137fd3481658); corrected candidate 5e5c5dd9081b against a0952cf. Config read: FLOW.codex.md — FLOW_CONTEXT=unattended: FLOW.unattended.md not found.
- Local main still points to the older study base; origin/main is the fetched base incorporated before implementation. No base integration occurred during review.
- Tier: proportional; full diff L, recorded size M, effective M at high effort. Corrective delta S (525 lines, 9 files), high effort. No production API, schema, authentication, payment or personal-data contract changed; no sensitive-path bump.
- Built-in code-review is unavailable in this harness. No project review_skill/reviewers/exec_cmd is configured. Independent general-purpose correctness reviewers, contract extraction and targeted completeness reviewers provided the fallback; no configured panel member was replaced.
- Agent models: models.agents/models.workers from FLOW.codex.md; no model inherited by accident. Cost: 10/12 review tasks across correctness, contract, performance and two completeness rounds; monetary cost not reported.
- Final reviewed candidate: 0e65d3a37a81561bbc8715ed96271d7cdf59a4a9. Critical findings remaining: 0. Native Windows runtime remains unverified and is a merge prerequisite, not a claim of this review.

## Findings and corrections
1. App paste tests could return success when the held-paste premise failed. The Linux-only helper now expects its declared boundary, asserts that holding succeeded, returns unit, and callers cannot silently skip their UI assertions. Direct boundary witnesses also expect the Linux boundary.
2. Socket-backed listener tests and two session-helper integration tests attempted the deliberately unsupported Windows endpoint. Added 24 individual Windows ignored counterparts and inventory rows; gated only the corresponding Unix helpers/imports. Pure predicates/state tests remain enabled. Core MSVC all-target Clippy passes after correction.
3. The shell negative control killed a shell after startup rather than preventing startup. Its child-only PATH and ComSpec now resolve to fresh missing executables; normal runs preserve inherited shell resolution. Portable fixtures assert both environments.
4. Process snapshots polled every 50 ms. They now poll at 250 ms with deadline checks before/after discovery and before shutdown queries.
5. Parent contract comparison tightened saved-tree checks for required leaf types, orientation, positive finite weights, finite sums, nesting and the existing 128-console bound. Fixtures reject missing orientation/weight, zero weight and same-direction nesting.

The second frozen review read 5e5c5dd. Its last completeness result found one macOS regression: a Unix-gated clipboard test now expected a Linux-only boundary. It was narrowed to Linux, then independently rechecked at 9c6f4e9 and the formatting-only final revision 0e65d3a37a81561bbc8715ed96271d7cdf59a4a9. No source changed while each reviewer read its candidate. The late completeness result and its indentation nit are recorded in round 3, after the earlier round record had already been written.

## Coverage and completeness
- First correctness pass: app per-test gates, mirrors, helper dependencies, early returns, plus core platform ancestry.
- Independent core pass: ConPTY bounded reads, listener/socket behavior, session helper, installer/launch, reporter and CI enforcement.
- Contract extraction was performed without the design, then compared with AC1–AC5 and the copied external contract. The stable Windows job identifier is retained; no public API changes.
- Parent independently reviewed Python inventory validation, fixtures, ignore baselines and compiler-harness identities, plus source manifest registrations.
- Completeness round 1 requested explicit functional review of app session/restoration/resume/opening/settings and core panel-contract/panel-control/term-core files. Fresh reviewers checked their full original diffs and found no production regressions or weakened assertions.
- Completeness round 2 requested explicit UI/viewer and associated integration-test review beyond platform ancestry. A targeted pass covered those paths; omission-only changes preserve existing assertions, and the boundary changes strengthen their premise.
- Areas not covered after two rounds: native Windows GUI/ConPTY execution cannot be observed on this Linux runner; carried explicitly into validation and publication. No remaining source-review gaps.
- A reviewer initially described a subset as additions-only and referred to an existing Windows drive-URI witness as new. The parent checked the complete diff and requested the omitted six test files explicitly; the final coverage statement uses the complete scope, not that preliminary statistic.

## Data-access duel
No queries or schema changes touched. No unresolved query verdict.

## Fit and conventions
The named ignored twins and strict compiler inventory are required by the approved omission contract. Normal installed startup has no product-only test mode. Cleanup, focus/PID ownership, deadlines and failure artifacts directly support the observed installer-crash regression; no speculative service or abstraction added. Repository formatting and commit conventions retained.

## Quality gates
Full workspace formatting and Clippy pass. Python and portable PowerShell contract fixtures pass. Compiler-backed Linux inventory passes with 379 entries across 28 harnesses. Final whole-suite result is recorded in 07-validation.md.

## Next step
Validate the frozen corrected candidate; publish only a draft with native Windows verification explicitly pending.
