---
id: asp_numbered-prereleases-preserve-debian-upgrade-order_jhyx8y
slug: numbered-prereleases-preserve-debian-upgrade-order
name: Numbered prereleases preserve Debian and SemVer upgrade ordering
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-11-09T08:41:44.785Z'
updated_at: '2026-11-09T08:41:44.785Z'
last_verified: '2026-11-09T08:41:44.785Z'
file_paths:
  - scripts/release.py
  - scripts/package-linux.sh
  - scripts/test_release.py
  - docs/02-decisions.md
symbols: []
content_hashes:
  docs/02-decisions.md: 'sha256:cc24a44f3636083ca737cd9e3e2adef5e0bb352fc563a47fa158d2e14a5ba604'
tags:
  - distribution
  - debian
  - semver
  - release
feature_id: feat_distribution_kb0x1w
---

## What it does

A plain SemVer hyphen-to-Debian-tilde conversion is restricted to stable MAJOR.MINOR.PATCH and
numbered alpha.N, beta.N, rc.N labels. General prerelease identifiers have different
ordering/equality in dpkg: dpkg sorts ~rc10 greater than ~rc2 although SemVer lexical rc10
precedes rc2, and dpkg equates ~alpha01 with ~alpha1. Accepting arbitrary labels can silently
prevent upgrades or merge release identities. Numeric fields must have no leading zeroes; build
metadata is outside this initial release vocabulary.

## Where it lives

scripts/release.py version()/deb_version(), scripts/package-linux.sh version guard,
scripts/test_release.py actual dpkg ordering and Bash guard tests; docs/02-decisions.md D76.
Direct dpkg --compare-versions reproductions and mutation tests in delivery343 confirmed the
mismatches.
