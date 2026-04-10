#!/usr/bin/env python3
"""
dedup_trajectory.py — Remove consecutive duplicate-role message anomalies from
trajectory files produced by the Claude Code session merger.

Two file formats are supported:

  • Structured JSON  {"messages": [...], "meta": {...}}
    Consecutive same-role messages (no intervening user/tool turn) are merged:
    their content arrays are concatenated into the first message and the
    redundant second message is removed.

  • JSONL  (one JSON object per line)
    Lines whose role matches the immediately preceding role are dropped.

Usage:
    # Fix a single file (writes <file>.deduped alongside the original):
    python scripts/dedup_trajectory.py sessions/bugfix-1.json

    # Explicit output path:
    python scripts/dedup_trajectory.py sessions/develop-1.json out/develop-1-clean.json

    # Batch — all *.json / *.jsonl in a directory (in-place, *.bak originals kept):
    python scripts/dedup_trajectory.py --dir sessions/
"""

from __future__ import annotations

import argparse
import json
import shutil
import sys
from pathlib import Path


# ── Helpers ────────────────────────────────────────────────────────────────────

def _role(msg: dict) -> str | None:
    """Return the conversation role of a message dict, or None."""
    return msg.get("role") or msg.get("type") or None


def _is_convo_role(role: str | None) -> bool:
    return role in ("user", "assistant")


# ── Structured-JSON format ({"messages": [...], "meta": {...}}) ────────────────

def _dedup_structured(data: dict) -> tuple[dict, int]:
    """
    Merge consecutive same-role assistant messages.

    When two assistant messages appear back-to-back (no intervening user or
    tool message), their content arrays are concatenated into the first and
    the second is discarded.  Returns (new_data, n_merged).
    """
    messages: list[dict] = data.get("messages", [])
    out: list[dict] = []
    merged_count = 0
    prev_convo_role: str | None = None

    for msg in messages:
        role = _role(msg)

        if _is_convo_role(role) and role == prev_convo_role:
            # Merge: append this message's content into the previous same-role msg.
            # Handles both consecutive assistant AND consecutive user anomalies.
            prev = out[-1]
            prev_content = prev.get("content", [])
            this_content = msg.get("content", [])

            if isinstance(prev_content, list) and isinstance(this_content, list):
                prev["content"] = prev_content + this_content
            elif isinstance(prev_content, str) and isinstance(this_content, str):
                prev["content"] = prev_content + "\n" + this_content
            # else: incompatible types — keep previous, skip this
            merged_count += 1
            continue

        out.append(msg)
        if _is_convo_role(role):
            prev_convo_role = role
        elif role == "tool":
            prev_convo_role = role  # tool resets the consecutive-check

    return {**data, "messages": out}, merged_count


def process_structured(src: Path, dst: Path) -> tuple[int, int]:
    """Return (original_msg_count, merged_count)."""
    with src.open(encoding="utf-8", errors="replace") as fh:
        data = json.load(fh)

    original_count = len(data.get("messages", []))
    new_data, merged = _dedup_structured(data)

    with dst.open("w", encoding="utf-8") as fh:
        json.dump(new_data, fh, ensure_ascii=False, separators=(",", ":"))

    return original_count, merged


# ── JSONL format (one JSON object per line) ────────────────────────────────────

def process_jsonl(src: Path, dst: Path) -> tuple[int, int]:
    """Return (total_lines, dropped_lines)."""
    kept: list[str] = []
    dropped = 0
    last_role: str | None = None

    with src.open(encoding="utf-8", errors="replace") as fh:
        for raw in fh:
            stripped = raw.rstrip("\n")
            if not stripped:
                kept.append(stripped)
                continue
            try:
                obj = json.loads(stripped)
            except json.JSONDecodeError:
                kept.append(stripped)
                continue

            role = _role(obj)

            if _is_convo_role(role) and role == last_role:
                dropped += 1
                continue

            if role is not None:
                last_role = role
            kept.append(stripped)

    dst.write_text("\n".join(kept) + "\n", encoding="utf-8")
    return len(kept) + dropped, dropped


# ── File dispatcher ────────────────────────────────────────────────────────────

def _is_jsonl(path: Path) -> bool:
    """
    Detect whether a file uses the structured-JSON format {"messages":[...], "meta":{...}}
    or plain JSONL (one JSON object per line).

    Loads the whole file as a single JSON document; if it has a top-level
    "messages" key it is the structured format.  Any other outcome (parse
    error or missing key) means JSONL.
    """
    try:
        with path.open(encoding="utf-8", errors="replace") as fh:
            obj = json.load(fh)
        return "messages" not in obj if isinstance(obj, dict) else True
    except (json.JSONDecodeError, OSError):
        return True  # could not parse as single JSON doc -> JSONL


def process_file(src: Path, dst: Path) -> str:
    """Process one file; return a human-readable result line."""
    if _is_jsonl(src):
        total, changed = process_jsonl(src, dst)
        verb = "lines dropped"
    else:
        total, changed = process_structured(src, dst)
        verb = "messages merged"

    if changed == 0:
        status = "clean"
    else:
        status = f"{changed} {verb}"
    return f"  {src.name}  ->  {dst.name}  [{total} messages, {status}]"


# ── CLI ────────────────────────────────────────────────────────────────────────

def main() -> None:
    parser = argparse.ArgumentParser(
        description="Remove consecutive duplicate-role message anomalies from trajectory files.",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=__doc__,
    )
    parser.add_argument("source", nargs="?", help="Path to a single trajectory file.")
    parser.add_argument("output", nargs="?", help="Output path (default: <source>.deduped).")
    parser.add_argument(
        "--dir",
        metavar="DIRECTORY",
        help="Process all *.json / *.jsonl files in a directory in-place (originals backed up as *.bak).",
    )
    args = parser.parse_args()

    if args.dir:
        d = Path(args.dir)
        if not d.is_dir():
            sys.exit(f"error: {d} is not a directory")
        files = sorted(d.glob("*.json")) + sorted(d.glob("*.jsonl"))
        if not files:
            print(f"No trajectory files found in {d}")
            return
        for src in files:
            if ".deduped" in src.stem or src.suffix == ".bak":
                continue
            bak = src.with_suffix(src.suffix + ".bak")
            shutil.copy2(src, bak)
            result = process_file(src, src)  # in-place
            print(result.replace(f"  {src.name}", f"  {src.name} [bak: {bak.name}]", 1))
        return

    if not args.source:
        parser.print_help()
        sys.exit(1)

    src = Path(args.source)
    if not src.exists():
        sys.exit(f"error: {src} not found")

    suffix = src.suffix or ".json"
    dst = Path(args.output) if args.output else src.with_suffix(f".deduped{suffix}")
    print(process_file(src, dst))


if __name__ == "__main__":
    main()
