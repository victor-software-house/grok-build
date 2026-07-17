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

## Build and CI policy (prefer workflows)

**Default: do not build this monorepo on the operator laptop.** Full or release builds are heavy; offload them to **GitHub Actions on standard GitHub-hosted runners** (public repos get free Actions minutes — use the free tier, **not** larger/paid runner labels).

| Prefer | Avoid |
|:--|:--|
| Push a branch / open a PR and let workflows run on **GitHub-hosted** runners | `cargo build --release` or full-workspace compiles on the local machine |
| `workflow_dispatch` on [`.github/workflows/build-macos-arm64.yml`](.github/workflows/build-macos-arm64.yml) for verify/publish | Installing heavy toolchains “just to compile once” locally |
| Standard labels (`ubuntu-latest`, `macos-*` free hosted images as defined in workflows) | Self-hosted runners, `*-large` / bigger paid runner SKUs unless an operator explicitly opts in |
| Artifact / release download for binaries | Long local `target/` trees for this crate graph |

### How agents should validate changes

1. **Edit and review** code locally (or remotely via a tool daemon when available).
2. **Commit and push**; trigger or rely on **GitHub Actions** for compile, test packaging, and release packaging.
3. **Watch the workflow run** (`gh run list` / `gh run watch`) and fix from logs — do not fall back to a full local build because CI is “inconvenient.”
4. Use a **local** `cargo check -p <crate>` / `cargo test -p <crate>` only when **strictly necessary** for a tiny, crate-scoped edit and the operator has asked for a quick loop. Even then:
   - Prefer `-p <crate>` over workspace-wide builds.
   - Prefer `cargo check` over `cargo build --release`.
   - Do not start a release build or full monorepo compile without explicit operator approval.

### Existing workflow (this fork)

- [`.github/workflows/build-macos-arm64.yml`](.github/workflows/build-macos-arm64.yml) — `workflow_dispatch` on a standard hosted `macos-*` runner: build from a pinned upstream boundary, package, optional prerelease publish.
- Trigger example:

```sh
gh workflow run build-macos-arm64.yml -f publish=false -f version=v0.0.0-ci
gh run watch
```

When adding CI: keep jobs on **GitHub-hosted free-tier runners**, cache deps where helpful, and avoid large/self-hosted runners unless documented and approved.

### Local commands (exception path only)

If a local cargo invocation is explicitly required:

```sh
cargo check -p xai-grok-pager-bin   # prefer check over build
cargo test -p <crate>
cargo clippy -p <crate>
cargo fmt --all
```

Requirements for local use: Rust via [`rust-toolchain.toml`](rust-toolchain.toml), [DotSlash](https://dotslash-cli.com) on `PATH` (for `bin/protoc`). Config: root `clippy.toml`, `rustfmt.toml`.

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

## Commits: Conventional Commits

All commits on this fork use **[Conventional Commits](https://www.conventionalcommits.org/)** (v1.0.0).

Format:

```text
<type>[optional scope]: <description>

[optional body]

[optional footer(s)]
```

| Type | Use for |
|:--|:--|
| `feat` | New user-facing capability |
| `fix` | Bug fix |
| `docs` | Documentation only |
| `ci` | CI/CD, workflows, release automation |
| `build` | Build system, deps that affect build |
| `refactor` | Code change that is not feat/fix |
| `test` | Tests only |
| `chore` | Maintenance that does not fit above |
| `perf` | Performance improvement |

Rules:

- **Description:** imperative, lowercase start preferred, no trailing period required; keep the first line ≤ ~72 characters.
- **Scope (optional):** short area, e.g. `docs(agents): …`, `ci(macos): …`, `feat(tool-protocol): …`.
- **Breaking changes:** `feat!:` / `fix!:` or a `BREAKING CHANGE:` footer.
- Squash merges use the **PR title** as the default commit subject — PR titles must also be conventional.

Examples:

```text
feat(tool-protocol): add session bind handshake
fix(pager): restore terminal modes on child death
docs(agents): require explicit approval before merging PRs
ci: prefer GitHub-hosted runners for release builds
```

## Branches: conventional type prefixes

Branch names follow the same **type vocabulary** as Conventional Commits (often called **conventional branch names** or **type-prefixed branches**).

```text
<type>/<short-kebab-description>
```

| Pattern | Example |
|:--|:--|
| `feat/…` | `feat/tool-daemon-listen` |
| `fix/…` | `fix/cache-key-collision` |
| `docs/…` | `docs/agents-pr-merge-policy` |
| `ci/…` | `ci/macos-arm64-release` |
| `refactor/…` | `refactor/tool-runtime-split` |
| `chore/…` | `chore/sync-upstream-main` |
| `test/…` | `test/tool-protocol-handshake` |

Rules:

- One primary type prefix; lowercase `kebab-case` after the slash.
- No spaces, no personal names, no ticket-only names like `my-branch` or `temp`.
- Optional: `type/scope-description` when helpful (`feat/tool-protocol-session-bind`).
- Default branch stays `main`. Do not commit product work directly to `main` unless the operator says so.

## Pull requests and merge policy

Agents may **open** PRs when that is part of the task. Agents must **not** merge (or enable auto-merge) unless the operator has **explicitly approved that merge in the current conversation**.

| Allowed without extra approval | Requires explicit operator approval |
|:--|:--|
| Create a branch, commit, push | `gh pr merge`, merge via API, or “merge when green” |
| Open a PR (`gh pr create` / REST) | Squash/rebase/merge of that PR |
| Update PR description, re-request CI | Force-push to default branch |
| Report PR URL and wait | Closing/reopening PRs in a way that lands code on `main` |

### Process (default)

1. Implement on a feature branch; push to `origin`.
2. Open a PR targeting `main` (or the branch the operator named).
3. **Stop.** Paste the PR URL and a short summary of what changed and what CI is expected to do.
4. Wait for the operator to say to merge (or to request changes).
5. Only after that approval: merge using the method they prefer (or ask if unspecified). Prefer **squash** for small doc-only PRs unless told otherwise.
6. Do **not** chain “open PR → immediately merge” in one turn.

“Open a PR”, “ship it when ready”, or “create the PR” alone is **not** merge approval. Merge only on clear language such as “merge it”, “merge #N”, or “approve and merge”.

## Remotes (typical fork layout)

| Remote | Role |
|:--|:--|
| `origin` | Working fork (push feature branches / releases here) |
| `upstream` | Public SpaceXAI tree (`xai-org/grok-build`) |

## Syncing from upstream

Upstream publishes periodic **“Synced from monorepo”** commits and updates root [`SOURCE_REV`](SOURCE_REV) (the monorepo SHA for this snapshot). Prefer a **fast-forward** of `main` when possible; never invent a parallel history of the upstream tree.

### Check whether upstream moved

```sh
git fetch upstream
git fetch origin
git log --oneline origin/main..upstream/main   # commits we are missing
git show upstream/main:SOURCE_REV              # new monorepo pin, if any
```

### Bring `main` up to date (no local fork commits ahead)

When `origin/main` is strictly behind `upstream/main` (no unique fork commits on `main` except already-merged work):

```sh
git checkout main
git merge --ff-only upstream/main
git push origin main
```

If `git merge --ff-only` refuses, **stop** and inspect: either rebase/replay fork-only commits on top of the new upstream tip, or open a sync PR instead of rewriting.

### When this fork has commits on `main` (CI, docs, …)

1. `git fetch upstream && git checkout main && git merge --ff-only upstream/main` if possible.
2. If not FF-able: merge or rebase **fork-only** commits onto `upstream/main` on a branch (e.g. `chore/sync-upstream`), push, open a PR, let **GitHub-hosted CI** validate (see build policy above).
3. Do **not** force-push `main` unless an operator explicitly allows it and org branch rules permit (this org may block force-pushes to the default branch).

### After sync

- Confirm [`SOURCE_REV`](SOURCE_REV) matches the intended upstream snapshot.
- Re-run / dispatch fork workflows if packaging or release artifacts depend on the new pin.
- Prefer CI over a local full rebuild (see **Build and CI policy**).

## Companion files

| File | Role |
|:--|:--|
| [`AGENTS.md`](AGENTS.md) | This guide (canonical for coding agents) |
| [`CLAUDE.md`](CLAUDE.md) | One-line import of this file for Claude-compatible tools |

## When stuck

1. Read crate-level docs and `README.md` development section.
2. Prefer GitHub Actions over local monorepo builds.
3. Prefer extending existing protocol/runtime crates over new top-level systems.
