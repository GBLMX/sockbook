# sockbook

Linux socket inspector with an offline port knowledge base.

`sockbook` combines live TCP/UDP socket data from the kernel with an embedded
service knowledge base. The socket data is authoritative; the service names are
**hints only** and never asserted as facts.

## Build

```bash
cargo build --release
# optional: enable `sockbook db update` over HTTPS
cargo build --release --features web
```

Linux only. No root required: sockets are enumerated from `NETLINK_SOCK_DIAG`
via `netstat2`, and owner PIDs are resolved on a best-effort basis from
`/proc/<pid>/fd`. Processes owned by other users require higher privileges to
attribute; the socket itself is still listed.

## Usage

```bash
sockbook list                     # TCP + UDP, all states
sockbook list -l -t               # listening TCP
sockbook list --port 8000 --json
sockbook lookup 8000              # knowledge base only, no kernel access
sockbook search redis
sockbook browse --category database --limit 20
sockbook db update --file data/curated.json
sockbook db update                # requires --features web
```

`lookup` and `search` return exit code 1 when nothing matches. `db update`
validates the payload and writes it atomically to
`$XDG_CACHE_HOME/sockbook/ports.json` (default `~/.cache/sockbook/ports.json`).
User overrides live in `$XDG_CONFIG_HOME/sockbook/overrides.json` and are never
touched by updates.

## Knowledge base

- `data/iana.json` is generated from the IANA service-name/port registry by
  `scripts/gen_iana.py`. IANA dedicates protocol registry data to the public
  domain.
- `data/curated.json` is a small hand-maintained overlay for de-facto developer
  ports. IANA assignments are often misleading for these (for example 3000,
  8000, 8888, 9090, 9092, 9200).
- Merge precedence is entry-level: user overrides > downloaded cache > built-in
  (curated > IANA). If a downloaded entry disappears, the built-in entry returns.

Hints are labelled with their tier. A hint means "this port is conventionally
used by X", not "this process is X".

## Known limits

- No Recv-Q/Send-Q, listen backlog, cgroup, or extended TCP info; those need a
  lower-level netlink backend than `netstat2` exposes.
- UDP peer addresses are not exposed by `netstat2` 0.11; UDP rows show no peer
  and state `UNCONN`.
- Sockets are only visible in the current network namespace.
- Terminal output sanitizes control characters from command lines and knowledge
  base text. JSON output is raw and intended for machine consumption.

## License

MIT. See `LICENSE`. Curated data is CC0; IANA registry data is used under the
IANA/IETF public-domain dedication.
