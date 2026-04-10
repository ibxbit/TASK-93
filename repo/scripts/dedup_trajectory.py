#!/usr/bin/env python3
"""
dedup_trajectory.py — Remove consecutive duplicate-role messages from JSONL
trajectory files.

Consecutive assistant messages (or consecutive user messages) are artifacts of
the log-merging process when two session files are concatenated end-to-end.
This script keeps the first occurrence of each role in a run and silently drops
any that immediately follow with the same role before a role change.

Usage:
    # Single file — writes <input>.deduped.jsonl alongside the source:
    python scripts/dedup_trajectory.py sessions/develop-1.jsonl

    # Explicit output path:
    python scripts/dedup_trajectory.py sessions/develop-1.jsonl out/clean.jsonl

    # All *.jsonl in a directory (in-place, originals backed up as *.bak):
    python scripts/dedup_trajectory.py --dir sessions/
"""

from __future__ import annotations

import argparse
import json
import shutil
import sys
from pathlib import Path


def _message_role(obj: dict) -> str | None:
    """Extract the conversation role from a parsed trajectory line, or None."""
    # Claude Code JSONL lines carry role in the top-level "type" field …
    msg_type = obj.get("type")
    if msg_type in ("user", "assistant"):
        return msg_type
    # … or nested inside a "message" object.
    message = obj.get("message")
    if isinstance(message, dict):
        role = message.get("role")
        if role in ("user", "assistant"):
            return role
    return None


def dedup_file(src: Path, dst: Path) -> tuple[int, int]:
    """
    Read *src*, deduplicate consecutive same-role messages, write to *dst*.

    Returns ``(total_lines_read, lines_dropped)``.
    """
    kept: list[str] = []
    dropped = 0
    last_role: str | None = None

    with src.open(encoding="utf-8") as fh:
        for raw in fh:
            stripped = raw.rstrip("\n")
            if not stripped:
                kept.append(stripped)
                continue

            try:
                obj = json.loads(stripped)
            except json.JSONDecodeError:
                # Non-JSON lines (e.g. comments) are preserved unchanged.
                kept.append(stripped)
                continue

            role = _message_role(obj)

            if role is not None and role == last_role:
                dropped += 1
                continue

            if role is not None:
                last_role = role

            kept.append(stripped)

    dst.write_text("\n".join(kept) + "\n", encoding="utf-8")
    return len(kept) + dropped, dropped


def process_single(src: Path, dst: Path | None) -> None:
    if dst is None:
        dst = src.with_suffix(".deduped.jsonl")
    total, dropped = dedup_file(src, dst)
    status = "✓" if dropped == 0 else f"⚠  {dropped} duplicate(s) dropped"
    print(f"  {src.name}  →  {dst.name}  [{total} lines, {status}]")


def process_directory(directory: Path) -> None:
    files = sorted(directory.glob("*.jsonl"))
    if not files:
        print(f"No .jsonl files found in {directory}")
        return
    for src in files:
        if src.suffix == ".jsonl" and ".deduped" not in src.stem:
            bak = src.with_suffix(".jsonl.bak")
            shutil.copy2(src, bak)
            total, dropped = dedup_file(src, src)  # in-place
            status = "✓" if dropped == 0 else f"⚠  {dropped} duplicate(s) dropped"
            print(f"  {src.name}  [backed up → {bak.name}, {total} lines, {status}]")


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Remove consecutive duplicate-role messages from JSONL trajectories.",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=__doc__,
    )
    parser.add_argument(
        "source",
        nargs="?",
        help="Path to a single .jsonl file.",
    )
    parser.add_argument(
        "output",
        nargs="?",
        help="Output path (default: <source>.deduped.jsonl).",
    )
    parser.add_argument(
        "--dir",
        metavar="DIRECTORY",
        help="Process all *.jsonl files in a directory in-place (originals backed up as *.bak).",
    )
    args = parser.parse_args()

    if args.dir:
        d = Path(args.dir)
        if not d.is_dir():
            print(f"error: {d} is not a directory", file=sys.stderr)
            sys.exit(1)
        process_directory(d)
        return

    if not args.source:
        parser.print_help()
        sys.exit(1)

    src = Path(args.source)
    if not src.exists():
        print(f"error: {src} not found", file=sys.stderr)
        sys.exit(1)

    dst = Path(args.output) if args.output else None
    process_single(src, dst)


if __name__ == "__main__":
    main()
