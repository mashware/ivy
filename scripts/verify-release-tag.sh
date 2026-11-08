#!/usr/bin/env bash
# Verify the current remote tag, not the ref cached when the workflow started.
set -euo pipefail
[[ $# == 2 ]] || { echo 'Usage: verify-release-tag.sh TAG COMMIT' >&2; exit 2; }
tag=$1
expected=$2
[[ $expected =~ ^[0-9a-f]{40}$ ]] || { echo 'Expected a lowercase commit SHA' >&2; exit 1; }
git check-ref-format "refs/tags/$tag"
git fetch --quiet --no-tags origin "refs/tags/$tag"
actual=$(git rev-parse --verify 'FETCH_HEAD^{commit}')
[[ $actual == "$expected" ]] || {
    echo 'The remote release tag no longer names the approved commit.' >&2
    exit 1
}
