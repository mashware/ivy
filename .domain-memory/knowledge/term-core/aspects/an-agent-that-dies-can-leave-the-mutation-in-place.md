---
id: asp_an-agent-that-dies-can-leave-the-mutation-in-place_q1bpsn
slug: an-agent-that-dies-can-leave-the-mutation-in-place
name: >-
  A test agent that dies halfway can leave the mutation APPLIED in the working
  tree — and go on writing without appearing in the agent list
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-18T13:59:31.515Z'
updated_at: '2026-08-18T13:59:31.515Z'
last_verified: '2026-08-18T13:59:31.515Z'
file_paths:
  - crates/app/src/lib.rs
symbols:
  - absorb
content_hashes:
  crates/app/src/lib.rs: 'sha256:d7688143658df12a548b8eb3bd4e0e7fc0ce9966b7a5e14a6e7d7a8e8835ba9d'
tags:
  - method
  - tests
  - mutation
  - agents
feature_id: feat_term-core_nx0g1r
---

## What it does

**The project's hard rule is that tests are mutated**: what the test claims to cover is broken on
purpose and it is checked that the test falls. When a subagent does that, the defect lives **in the
working tree** for as long as the check lasts, and that is where the risk that is not obvious sits.

**What happened (F2.3, second PR).** A test agent died without delivering a report and left **three**
mutations applied. It went on patching **after** dying and **without appearing in the agent list any
more**; it had to be stopped the hard way. It was spotted by checking `git status` before closing the
session.

> **And one of the three SURVIVED the entire suite.** The weight scaling of `absorb` replaced by a
> copy of the inner weight passed the crate's **383** tests. Without that `git status` a PR would have
> been delivered with the defect **planted** and in green — not a coverage gap, a real defect inside the
> diff.

**The two lessons, and the second is the surprising one:**

1. **Checking `git status` before calling a session good is not hygiene, it is the last net.** An agent
   that mutates files and dies halfway leaves the code worse than it found it, and the suite in green
   does not give it away — precisely because the mutation that survives is the one no test pins down.
2. **The abandoned mutation is a DETECTOR of coverage gaps, and a good one.** Here it handed over the
   milestone's most valuable gap: that nothing was pinning the weights of `absorb` — see
   [[removing-a-leaf-never-takes-area-from-those-left]] and
   [[a-geometric-invariant-is-asserted-in-proportions]]. What a dead agent leaves in place deserves to
   be read before reverting it, not after.

**Practical consequence for this project:** a test written by an agent is **audited and mutated** before
being accepted, and the session is not closed without a clean `git status` nor without comparing against
a control copy of the file. This was already done in the validation of this same milestone's first PR,
where the agent wrote three good tests but likewise did not deliver a report.

## Where it lives

It happened over `crates/app/src/lib.rs` in the second PR of F2.3 (issue 76), during the validation
phase. Recorded in `07-validation.md` of that work with the table of the three abandoned mutations and
which of them survived.
