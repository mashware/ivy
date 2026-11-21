---
id: asp_gh-run-logs-mislabel-jobs_5p3gm5
slug: gh-run-logs-mislabel-jobs
name: >-
  gh run view --log can file one job's lines under another: read a failing job's
  log from the API before diagnosing it
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-11-21T18:35:17.237Z'
updated_at: '2026-11-21T18:35:17.237Z'
last_verified: '2026-11-21T18:35:17.237Z'
file_paths:
  - .github/workflows/ivy-prerelease.yml
symbols: []
content_hashes:
  .github/workflows/ivy-prerelease.yml: 'sha256:10bc02cffee7a35a00e562e2fb4fd6625b91eaf0a655c0c3efb3b6586e39554b'
tags:
  - ci
  - github
  - tooling
feature_id: feat_distribution_kb0x1w
---

## What it does

Seen on mashware/ivy on 21 November 2026 (runs 44915958667 and 44928690359).

`gh run view --log-failed` and `gh run view --log --job <id>` returned lines under the wrong
job. The Linux `checks` job showed the Windows omission report and Windows test failures,
with the wrong step names and identical timestamps. Diagnosing from that output would have
sent a fix to the wrong job. The real Linux failure was a different test.

**Read a failing job from the API:** `gh api repos/mashware/ivy/actions/jobs/<job id>/logs`,
with the job id from `gh run view <run> --json jobs`. Logs of older runs (for example
44844839493) came back empty about a day later, so save the evidence while it is there.

## Where it lives

.github/workflows/ivy-prerelease.yml (the jobs involved: checks, windows-startup-checks, windows).
