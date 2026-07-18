# Todo ledger

Fork-local work tracking. Technical only (no secrets, no client/personal data).


## Repo policy / branch protection

**Live rulesets (2026-07-17):**

| Ruleset | Scope | Rules |
|:--|:--|:--|
| Block force pushes (org) | `~DEFAULT_BRANCH` | `non_fast_forward` |
| Protect archive branches | `archive/**` | `deletion`, `non_fast_forward` |
| **Protect main** | `~DEFAULT_BRANCH` | `update`, `pull_request` (0 reviews), `required_status_checks` (`identity + commitlint`), author/committer email regexes; bypass: OrganizationAdmin |

- [x] Document and pin **whitelisted git identities** — [`config/commit-email-allowlist`](config/commit-email-allowlist)
- [x] CI guard on PRs/push — [`.github/workflows/policy.yml`](.github/workflows/policy.yml)
- [x] GitHub ruleset email patterns on default branch (Protect main)
- [x] Require Policy status + PR path on `main` (Protect main)
- [x] Restrict direct pushes (`update` + org-admin bypass)
- [x] Policy range harden: force-push-safe `before` (tip-only fallback + decision log)
- [x] `pr:merge`: required check-runs gate + REST merge (avoid `gh` BLOCKED preflight; `--admin` break-glass only)
- [ ] Signed commits: enable `required_signatures` on Protect main **after** SSH signing shows Verified on a real push
  - Prefer SSH signing (not GPG); public key must be a GitHub **signing** key
  - Local: `gpg.format=ssh`, `commit.gpgsign=true`, `user.signingkey`
  - Operator-specific setup: **private notes / agent memory only** (not this tree)

---

## Build model (facts — read this before changing CI)

### Is this a monolith?

**Yes, one Cargo workspace** (~79 members). Root `Cargo.toml` is **generated** (treat read-only).\
The ship binary is the **composition root** `xai-grok-pager-bin` → artifact `xai-grok-pager` / `grok`.\
`cargo build -p xai-grok-pager-bin` does **not** build all 79 crates, but it pulls a **large path-dep closure** (pager, shell, workspace, tools, config, …) plus crates.io. That is why cold builds feel monorepo-slow.

Upstream’s own README: always `cargo check -p <crate>`; full-workspace is slow.

### What is prebuilt vs compiled here?

| Artifact | Source |
|:--|:--|
| Rust crates / `xai-grok-pager` | **Built from this tree** every time (no prebuilt app binary in-repo) |
| [`bin/protoc`](bin/protoc) | DotSlash stub → downloads pinned protoc (not compiled by us) |
| `ripgrep` in CI release path | Downloaded pinned tarball (bundled into product build env) |
| Rust toolchain | [`rust-toolchain.toml`](rust-toolchain.toml) via rustup on the runner |

There is **no** “download grok binary and stamp it” path for our release workflow — we compile.

### What must run on macOS?

Only what needs a **Darwin/arm64 link + smoke**:

| Work | Runner |
|:--|:--|
| `cargo check` / `clippy` / unit tests (most crates) | **Linux is fine** (and free concurrency is higher) |
| `cargo build --release` **producing** Mach-O arm64 `xai-grok-pager` | **macOS arm64** hosted (or a Mac self-hosted) |
| Installer e2e (`uname` Darwin, `file` Mach-O, `otool`) | **macOS** |
| Pure packaging of an **already-built** artifact | Could be Linux if we only tar + checksum — today smoke assumes Darwin |

We do **not** need macOS to validate every PR. macOS is for **ship binaries** (and optional macOS-specific regressions later).

**Concurrency (public repo, org Team plan):** max **5 concurrent macOS** jobs org-wide; **60** total jobs; public minutes effectively free. Parallelism does not make one release cargo faster — it only helps multiple independent runs.

### Incremental: do we always rebuild everything?

| Layer | Behavior |
|:--|:--|
| **Cargo unit graph** | Already incremental: unchanged crates reuse `target/` if the cache is warm |
| **Our release job (historical)** | Cold or weak cache → feels like full rebuild every dispatch |
| **`--release` vs `check`** | Release optimizes whole graph (slow); `cargo check` is the PR default |
| **Workspace-wide** | Never the default for CI; always `-p <crate>` or an **affected** set |
| **True “only changed crates”** | Needs path filters + `cargo check -p …` list (or a small script); not free magic |

Goal: **PR = Linux + check/clippy + shared caches**. **Release = macOS + release binary**, rare, dispatch-only.

---

## Plan: fast CI (phased)

### Phase A — Trigger UX + lanes (now / next PR)

Split **three lanes** so release never blocks day-to-day work:

| Lane | When | Runner | Goal |
|:--|:--|:--|:--|
| **Policy** | every PR / push `main` | ubuntu | identity + commitlint (exists) |
| **PR rust** | PR (path-filtered) | ubuntu | `cargo check` / later clippy on pager-bin (or affected) |
| **Release** | `workflow_dispatch` only | macos-26 | `--release` binary + package + optional publish |

- [x] Release builds **this repo** at `source_ref` (branch/tag/SHA) — not upstream pin
- [x] Document lanes + monolith facts in this ledger
- [x] Harden [`ci:dispatch`](mise-tasks/ci/dispatch): refuse missing remote SHA, print run URL, optional `--watch`, `--no-package`
- [x] PR workflow: Linux `cargo check -p xai-grok-pager-bin` + rust cache (not `--release`) — [`.github/workflows/pr.yml`](.github/workflows/pr.yml)
- [x] Path filters on PR rust (Cargo/crates/mise CI paths)
- [x] AGENTS: lanes + “never use release workflow for compile-check”
- [ ] Require PR rust check on Protect main **after** it is green and stable (optional ruleset bump)
- [ ] First green PR run measured (cold vs warm); tune timeout if needed

### Phase B — Move YAML soup → mise **file tasks** + CI profile

Huge inline `run: |` blocks are hard to test locally. Convention (this repo / official mise):

- [`mise.toml`](mise.toml) — tools + **short** one-liner tasks only
- [`mise.ci.toml`](mise.ci.toml) — **config environment** (`MISE_ENV=ci`): CI env + `task_config.includes` (not fat `run` blobs)
- File tasks:
  - [`mise-tasks/`](mise-tasks/) — always (operator: `ci:dispatch`, `pr:merge`, …)
  - [`mise-tasks-ci/`](mise-tasks-ci/) — only when CI profile active (`ci:check-pager`, `ci:release-pager`, …)
- `task_config.includes` **replaces** defaults for that scope — re-list `mise-tasks` when adding `mise-tasks-ci`
- Base `mise.toml` is **never skipped** when `MISE_ENV=ci`
- Headers: `#MISE description=…`, `#USAGE` for args
- `depends` does **not** pass env to dependents — source `tools` in-process when `PROTOC` must stick
- Workflows: `MISE_ENV=ci` + `mise run ci:check-pager`

- [x] CI profile: `mise.ci.toml` + `mise-tasks-ci/ci/*` runner tasks
- [x] Operator `ci:dispatch` / `ci:watch` stay under `mise-tasks/ci/`
- [x] Wire `jdx/mise-action` + `MISE_ENV=ci` in PR + release workflows
- [x] `workflows:lint` (actionlint) on workflow changes; shellcheck clean enough on new scripts
- [ ] Local smoke: `mise -E ci run ci:check-pager` (long first time; optional)

### Phase E notes (partial)

- [x] Release **dispatch-only** + `package` input / `ci:dispatch --no-package` (compile-only path exists)

### Phase C — Caches that actually hit

- [ ] Shared **deps** cache key: `hashFiles('Cargo.lock')` + rust version (OS-specific)
- [ ] **Target** cache: PR uses softer restore-keys (branch-agnostic prefix); release keys include `SOURCE_SHA` for exact hit
- [ ] Measure first vs second PR run wall time; note in ledger
- [ ] Consider `sccache` (optional) if target cache is flaky on macOS size limits (6 GiB save guard already exists)
- [ ] Document what **does not** cache (toolchain download, protoc zip) and pin those permanently

### Phase D — Incremental / affected crates

- [ ] Script or task: given `git diff --name-only base...HEAD`, map paths → cargo packages (crate root ownership)
- [ ] PR job: `cargo check -p pkg1 -p pkg2 …` for affected; always include `xai-grok-pager-bin` if any codegen/common in its closure changed (conservative fallback: check pager-bin only)
- [ ] Optional: `cargo check -p xai-tool-protocol` etc. when only `crates/common/**` changes
- [ ] Never `cargo check --workspace` on free PR CI unless we introduce a nightly/manual job
- [ ] Clippy on affected set after check is stable (`-D warnings` may need a grace period)

### Phase E — Faster release binary (macOS only when needed)

- [x] Keep release **dispatch-only** (no PR `--release`)
- [x] Job split: **build** (macOS) → **package** (optional) → **publish** (optional)
- [x] Skip package when `package=false` / `ci:dispatch --no-package`
- [ ] Revisit `fetch-depth: 0` — only when `source_ref` is a non-tip SHA; branch tips can use depth 1
- [ ] Cache warm-up workflow (manual) that only builds deps for `pager-bin` on macos-26 / ubuntu
- [ ] Do **not** matrix macOS × many crates on free tier (5 macOS cap)

### Phase F — Cross-compile research (maybe never)

- [ ] Spike: Linux → `aarch64-apple-darwin` with osxcross / cargo-zigbuild (likely painful, not free-tier friendly)
- [ ] Decision record: stay on `macos-26` for ship **or** invest in self-hosted Mac
- [ ] Self-hosted only if operator approves (org policy: free hosted first)

---

## CI / builds (checkbox index)

### Lanes

- [x] macOS release workflow builds **this repo** at chosen `source_ref`
- [x] Harden `ci:dispatch` (remote exists, URL, `--watch`, `--no-package`)
- [x] Linux PR `cargo check -p xai-grok-pager-bin` (debug, cached)
- [x] PR path filters
- [ ] Optional: PR clippy pager-bin (`ci:clippy-pager` task exists)
- [ ] Optional: PR `cargo test -p` for small crates only (not full pager e2e)
- [ ] Stay on free GitHub-hosted SKUs (no large/self-hosted unless approved)
- [ ] Protect main: add PR rust check name when stable

### mise CI extraction

- [x] `mise.ci.toml` config environment + non-additive `includes`
- [x] Runner file tasks under `mise-tasks-ci/ci/`
- [x] Operator `ci:dispatch` / `ci:watch` under `mise-tasks/ci/`
- [x] Workflows: `MISE_ENV=ci` + `mise run ci:*`
- [ ] Further shrink residual YAML (raw staging / publish notes) if desired

### Cache + incremental

- [ ] Shared Actions cache (deps + target); document hit rates
- [ ] Affected-crate mapping for PR
- [x] Release compile-only mode (skip package) — `package` input / `ci:dispatch --no-package`
- [ ] Shallow fetch when building branch tips

### Upstream sync (automation)

Manual FF / sync-PR is documented in [`AGENTS.md`](AGENTS.md). Automate next:

- [ ] Scheduled (or manual-dispatch) workflow that fetches `upstream/main`, opens a **sync PR** into `main` when behind
  - Prefer fast-forward branch when history allows; otherwise merge/rebase branch + PR (never force-push `main`)
  - Use `pull_request` path so Policy + PR rust run
  - Label / title convention e.g. `chore(sync): upstream main @ <short-sha>`
  - Do not auto-merge without operator approval
- [ ] After merge: confirm [`SOURCE_REV`](SOURCE_REV) still matches the public export’s provenance note
- [ ] Optional: notify operator (issue comment only — no secrets/webhooks required for MVP)

---

## Remote tool execution

- [ ] Host daemon MVP: same tool handlers as local, stream over RPC (not per-call SSH pipes); prefer `xai-tool-protocol` / tool-runtime
- [ ] Client routing to the daemon (loopback + tunnel first)
- [ ] Loopback integration tests (handshake, bash stream, read/write)
- [ ] Bootstrap notes/script without secrets in the tree

## Local tooling

- [x] `mise.toml` + `mise.lock` (min_version 2026.7.7; lefthook, actionlint, shellcheck, dotslash; crate-scoped cargo tasks; CI dispatch)
- [x] `.envrc` → `use_mise_env` (direnv, no `mise activate`)
- [x] Local overrides in `.gitignore` (`mise.local.toml`, `lefthook-local.yml`, `.envrc.*`, `.env*`, …)
- [x] `lefthook.yml` thin callers (`commit-msg` commitlint, `post-checkout` worktree setup; identity is CI-only)
- [x] CI policy workflow (`identity:check` + `commitlint:range`)
- [x] CI profile (`mise.ci.toml` + `mise-tasks-ci/`) — see Phase B

## Docs

- [x] Root `AGENTS.md` + `CLAUDE.md`
- [x] Prefer GitHub-hosted CI over local monorepo builds
- [x] Conventional Commits + type-prefixed branches (brief)
- [x] Explicit operator approval before merge
- [x] Merge method default + archive/** retention
- [x] Local tooling (mise/direnv/lefthook) documented in `AGENTS.md`
- [x] `main` protection + signed-commits plan (public, vendor-neutral)
- [x] Document CI lanes (policy / PR rust / release) in `AGENTS.md`
- [ ] Keep this ledger current

## Upstream

- [ ] Periodic sync from `upstream` (manual FF when possible; else sync PR + CI — see `AGENTS.md`)
- [ ] Automate that path — see [Upstream sync (automation)](#upstream-sync-automation)

---

## Open backlog (technical)

Items still open. Completed Policy range + `pr:merge` REST gate live on this PR branch (merge to `main` pending).

| Area | Open work | Constraint / note |
|:--|:--|:--|
| **Protect main** | Optionally require check context `cargo check pager-bin` | Only after cold/warm timings are acceptable; keep Policy as the hard gate until then |
| **PR CI** | Wire `ci:clippy-pager` (and optional small-crate tests) into [`.github/workflows/pr.yml`](.github/workflows/pr.yml) | Task exists under `mise-tasks-ci/`; not in the workflow matrix yet |
| **Cache** | Document Actions deps/target hit rates; tune keys | Phase C |
| **Incremental** | Path → crate map for affected `cargo check -p …` | Phase D; fallback remains full pager-bin check |
| **Release** | Shallow `fetch-depth` for branch tips; optional warm-up workflow | Phase E remainder; `package=false` path already exists |
| **YAML** | Further shrink residual inline steps in release workflow | Staging/publish notes still in YAML |
| **Lab** | Keep `sandbox/merge-lab` + ruleset **Protect sandbox/merge-lab** for merge/CI experiments | Not a production branch; not `main` |
| **Upstream** | Scheduled/manual sync workflow → open sync PR only | Never auto-merge; confirm `SOURCE_REV` after merge |
| **Product** | Remote tool daemon MVP (RPC, loopback tests, bootstrap without secrets) | Prefer existing tool-protocol/runtime crates |
| **Signing** | SSH commit signing → GitHub Verified → then `required_signatures` on Protect main | Operator machine config stays out of this tree |
| **Repo topology** | Detach public fork → non-fork remote (deferred) | Cosmetic/parent-default issue; does not fix `gh pr merge` preflight |
| **Upstream CLI** | Track [cli/cli#13388](https://github.com/cli/cli/issues/13388) | Local workaround: check-runs gate + REST merge; `--admin` break-glass only |
