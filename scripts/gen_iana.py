#!/usr/bin/env python3
"""Generate data/iana.json from the IANA service-names-port-numbers CSV.

Usage:
    scripts/gen_iana.py                       # fetch from iana.org
    scripts/gen_iana.py --input /tmp/ports.csv
    scripts/gen_iana.py --output /tmp/iana.json

The output is deterministic for the same input bytes and source mtime.
"""

from __future__ import annotations

import argparse
import csv
import datetime as dt
import hashlib
import io
import json
import os
import sys
import urllib.request
from email.utils import parsedate_to_datetime

SOURCE = (
    "https://www.iana.org/assignments/service-names-port-numbers/"
    "service-names-port-numbers.csv"
)
MAX_PORT = 49151
LICENSE = "IANA protocol registries: public domain dedication"


def timestamp_for_file(path: str) -> str:
    mtime = os.path.getmtime(path)
    return (
        dt.datetime.fromtimestamp(mtime, tz=dt.timezone.utc)
        .replace(microsecond=0)
        .isoformat()
        .replace("+00:00", "Z")
    )


def read_input(path: str | None) -> tuple[bytes, str]:
    if path:
        with open(path, "rb") as fh:
            return fh.read(), timestamp_for_file(path)
    req = urllib.request.Request(SOURCE, headers={"User-Agent": "sockbook-gen-iana/1"})
    with urllib.request.urlopen(req, timeout=30) as resp:
        data = resp.read()
        last_modified = resp.headers.get("Last-Modified")
        generated_at = None
        if last_modified:
            parsed = parsedate_to_datetime(last_modified)
            generated_at = (
                parsed.astimezone(dt.timezone.utc)
                .replace(microsecond=0)
                .isoformat()
                .replace("+00:00", "Z")
            )
        if not generated_at:
            generated_at = (
                dt.datetime.now(tz=dt.timezone.utc)
                .replace(microsecond=0)
                .isoformat()
                .replace("+00:00", "Z")
            )
        return data, generated_at


def build_entries(raw: bytes) -> list[dict]:
    reader = csv.DictReader(io.StringIO(raw.decode("utf-8")))
    groups: dict[tuple[int, str], dict[str, set[str]]] = {}
    for row in reader:
        port_raw = (row.get("Port Number") or "").strip()
        proto = (row.get("Transport Protocol") or "").strip().lower()
        name = (row.get("Service Name") or "").strip()
        description = (row.get("Description") or "").strip()
        if not port_raw.isdigit() or proto not in ("tcp", "udp") or not name:
            continue
        port = int(port_raw)
        if not 1 <= port <= MAX_PORT:
            continue
        key = (port, description)
        group = groups.setdefault(key, {"protos": set(), "names": set()})
        group["protos"].add(proto)
        group["names"].add(name)

    entries = []
    for (port, description), group in sorted(groups.items()):
        names = sorted(group["names"], key=lambda n: (n.lower(), n))
        entries.append(
            {
                "port": port,
                "protos": sorted(group["protos"]),
                "name": names[0],
                "aliases": names[1:],
                "description": description,
            }
        )
    return entries


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--input", help="local IANA CSV (default: fetch)")
    parser.add_argument("--output", default="data/iana.json")
    args = parser.parse_args()

    raw, generated_at = read_input(args.input)
    entries = build_entries(raw)
    envelope = {
        "schema": 1,
        "generated_at": generated_at,
        "source": SOURCE,
        "source_sha256": hashlib.sha256(raw).hexdigest(),
        "license": LICENSE,
        "entries": entries,
    }
    os.makedirs(os.path.dirname(args.output) or ".", exist_ok=True)
    with open(args.output, "w", encoding="utf-8") as fh:
        json.dump(envelope, fh, ensure_ascii=False, separators=(",", ":"))
        fh.write("\n")
    print(f"wrote {len(entries)} entries to {args.output}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
