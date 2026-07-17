# Agent guide — grok-build

Technical notes for coding agents and automated tools working in this repository.
Humans should also read [`README.md`](README.md) and [`CONTRIBUTING.md`](CONTRIBUTING.md).

## What this repo is

- Rust monorepo for the **Grok Build** CLI/TUI (`grok`) and agent runtime.
- Published fork of the SpaceXAI public tree; `SOURCE_REV` records the upstream monorepo SHA for the synced snapshot.
- **Default branch:** `main`. Prefer small, reviewable commits on feature branches.

This file is **not** a product surface for end-user project `AGENTS.md` files that Grok itself may load from *other* working trees. It only guides work **on this codebase**.

## Public-tree rules

- Commit **technical** content only: architecture, build/test, protocols, generic remote-execution design.
- **Do not** commit secrets, tokens, credentials, private hostnames, client/employer operational detail, personal paths, or personal contact data.
- **Do not** open “contributor CLA” style contribution flows; see [`CONTRIBUTING.md`](CONTRIBUTING.md) (upstream does not accept unsolicited external PRs). Security reports: [`SECURITY.md`](SECURITY.md).

## Layout (high signal)

| Area | Path / crates |
|:--|:--|
| TUI / CLI entry | `xai-grok-pager-bin`, `xai-grok-pager` |
| Agent / shell runtime | `xai-grok-shell`, `xai-grok-agent` |
| Tool schemas / runtime | `xai-tool-types`, `xai-tool-runtime` |
| Tool wire protocol (Computer Hub) | `xai-tool-protocol` |
| Workspace / permissions | `xai-grok-workspace` |
| Config | `xai-grok-config`, `xai-grok-config-types` |
| CI (this fork) | `.github/workflows/build-macos-arm64.yml` |
| Install helper (this fork) | `scripts/install-github-release.sh` |

Prefer `cargo … -p <crate>` over full-workspace builds; workspace compiles are large.

## Build and check

Requirements: Rust via [`rust-toolchain.toml`](rust-toolchain.toml), [DotSlash](https://dotslash-cli.com) on `PATH` (for `bin/protoc`), then:

```sh
cargo check -p xai-grok-pager-bin
cargo build -p xai-grok-pager-bin --release
cargo test -p <crate>
cargo clippy -p <crate>
cargo fmt --all
```

Config: root `clippy.toml`, `rustfmt.toml`.

## Remote tool execution (design intent)

Goal: run the **same** tool operations (read, edit, list, grep, shell, …) on a **remote host** via a **persistent daemon** that streams results over a long-lived RPC channel—**not** by wrapping each tool call in ad-hoc remote shell one-liners.

| Prefer | Avoid |
|:--|:--|
| Daemon on the remote host executing shared tool handlers | Per-call `ssh host '…'` pipes as the execution model |
| Wire formats already in-tree (`xai-tool-protocol` / Computer Hub style session + tool call/progress) | Inventing a one-off side protocol without reusing existing crates |
| Loopback bind + authenticated tunnel (or equivalent) for MVP | Exposing unauthenticated tool RPC on the public internet |

Prior art outside this tree (for implementers): process/FS RPC daemons that stream over WebSocket or Unix socket (e.g. long-lived “exec server” patterns), separate from the interactive TUI process.

Implementation work should live in crates above (or a small dedicated host binary that reuses them), with tests that exercise **local loopback** first, then a real remote smoke.

## Git and identity

- Use the operator’s **configured** git identity for the environment you are in. Do not hardcode identities in commits or in this file.
- Do not rewrite published history unless an operator explicitly requests it and branch rules allow it.
- Keep secrets out of git; use local env / secret managers outside the tree.

## Remotes (typical fork layout)

| Remote | Role |
|:--|:--|
| `origin` | Working fork (push feature branches / releases here) |
| `upstream` | Public SpaceXAI tree (`xai-org/grok-build`) |

```sh
git fetch upstream
git log --oneline origin/main..upstream/main   # check for new upstream syncs
```

## When stuck

1. Read crate-level docs and `README.md` development section.
2. Scope to a single crate with `cargo check -p` / `cargo test -p`.
3. Prefer extending existing protocol/runtime crates over new top-level systems.
