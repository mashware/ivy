#!/usr/bin/env python3
"""Require the original Windows fixture regressions and their child witnesses to run and pass."""

import json
import re
import sys
from pathlib import Path


def main() -> int:
    expected = json.loads(Path(__file__).with_suffix('.json').read_text(encoding='utf-8'))
    output = Path(sys.argv[1]).read_text(encoding='utf-8-sig', errors='replace')
    outcomes = dict(re.findall(r'^test (\S+) \.\.\. (ok|FAILED|ignored[^\r\n]*)$', output, re.MULTILINE))
    missing = [name for name in expected if outcomes.get(name) != 'ok']
    if missing:
        for name in missing:
            print(f'Windows fixture regression did not run and pass: {name}', file=sys.stderr)
        return 1
    print(f'PASS all {len(expected)} named Windows fixture regressions and child witnesses')
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
