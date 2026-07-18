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
- [ ] Signed commits: enable `required_signatures` on Protect main **after** SSH signing shows Verified on a real push
  - Prefer SSH signing (not GPG); public key must be a GitHub **signing** key
  - Local: `gpg.format=ssh`, `commit.gpgsign=true`, `user.signingkey`
  - Operator-specific setup: **private notes / agent memory only** (not this tree)

## CI / builds

- [x] macOS release workflow builds **this repo’s dispatch ref** (`github.sha`) — not an upstream pin
- [ ] PR builds on standard GitHub-hosted runners (check/clippy scoped or light matrix) so work does not depend on local monorepo compiles
- [ ] Shared Actions cache across branches/PRs (deps + target; document what actually hits)
- [ ] Stay on free GitHub-hosted runner SKUs (no large/self-hosted unless approved)

## Upstream sync (automation)

Manual FF / sync-PR is documented in [`AGENTS.md`](AGENTS.md). Automate next:

- [ ] Scheduled (or manual-dispatch) workflow that fetches `upstream/main`, opens a **sync PR** into `main` when behind
  - Prefer fast-forward branch when history allows; otherwise merge/rebase branch + PR (never force-push `main`)
  - Use `pull_request` path so Policy + future PR CI run
  - Label / title convention e.g. `chore(sync): upstream main @ <short-sha>`
  - Do not auto-merge without operator approval
- [ ] After merge: confirm [`SOURCE_REV`](SOURCE_REV) still matches the public export’s provenance note (file comes from upstream; do not invent SHAs)
- [ ] Optional: notify operator (issue comment only — no secrets/webhooks required for MVP)

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

## Docs

- [x] Root `AGENTS.md` + `CLAUDE.md`
- [x] Prefer GitHub-hosted CI over local monorepo builds
- [x] Conventional Commits + type-prefixed branches (brief)
- [x] Explicit operator approval before merge
- [x] Merge method default + archive/** retention
- [x] Local tooling (mise/direnv/lefthook) documented in `AGENTS.md`
- [x] `main` protection + signed-commits plan (public, vendor-neutral)
- [ ] Keep this ledger current

## Upstream

- [ ] Periodic sync from `upstream` (manual FF when possible; else sync PR + CI — see `AGENTS.md`)
- [ ] Automate that path — see [Upstream sync (automation)](#upstream-sync-automation)
