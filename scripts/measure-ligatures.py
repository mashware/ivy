#!/usr/bin/env python3
"""Validate the declared ligature path, including its off and historical controls."""
import math
import re
import sys
from pathlib import Path


def validate(text, expected):
    def line(prefix, pattern, required=True):
        lines = [value for value in text.splitlines() if value.startswith(prefix)]
        if not lines and not required:
            return None
        if len(lines) != 1 or not (match := re.fullmatch(pattern, lines[0])):
            raise ValueError(f"missing, duplicate or malformed {prefix}")
        return match

    scene = line("Ligature scene:", r"Ligature scene: programming-ascii-v1 font=JetBrainsMono-v2\.304-Regular size=14\.7 scale=([0-9.]+) mode=(on|off)", expected != "none")
    if expected == "none" and scene:
        raise ValueError("historical batch unexpectedly selected a ligature scene")
    if scene:
        scale = float(scene[1])
        if not math.isfinite(scale) or scale <= 0:
            raise ValueError("invalid device scale")
        if expected != "auto" and scene[2] != expected:
            raise ValueError("ligature mode differs from the requested batch")
        expected = scene[2]
        line("Ligature startup:", r"Ligature startup: [0-9.]+ us \(explicit font read, validation and registration; before first frame\)")
    work = line("Ligature work:", r"Ligature work: ([0-9.]+) us \(included in painting; never subtracted twice\)", expected != "none")
    effects = line("Ligature effects:", r"Ligature effects: eligible_runs=([0-9]+) shaped_runs=([0-9]+) fallback_runs=([0-9]+) vertices=([0-9]+)", expected != "none")
    if work is None and effects is None:
        return  # Historical reports written before the optional path existed.
    if work is None or effects is None:
        raise ValueError("ligature work and effects must be declared together")
    spent = float(work[1])
    eligible, shaped, fallback, vertices = map(int, effects.groups())
    if not math.isfinite(spent) or spent < 0:
        raise ValueError("invalid included ligature cost")
    if expected == "on":
        if spent <= 0 or shaped == 0 or vertices == 0:
            raise ValueError("on mode did not witness actual shaped drawing and work")
        if eligible != shaped + fallback:
            raise ValueError("eligible runs do not match shaped plus fallback runs")
    elif any((spent, eligible, shaped, fallback, vertices)):
        raise ValueError("off/historical mode performed ligature work")


if __name__ == "__main__":
    try:
        validate(Path(sys.argv[1]).read_text(), sys.argv[2])
    except (ValueError, OSError, IndexError) as error:
        sys.exit(f"invalid ligature measurement: {error}")
