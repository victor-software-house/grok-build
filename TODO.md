# Todo ledger

Fork-local work tracking. Technical only (no secrets, no client/personal data).


## Repo policy / branch protection

Current live rulesets (2026-07-17): org **Block force pushes** on `~DEFAULT_BRANCH` (`non_fast_forward`); repo **Protect archive branches** on `archive/**` (`deletion` + `non_fast_forward`). No main PR gate, no identity patterns, no required signatures yet. API probe: `commit_author_email_pattern`, `committer_email_pattern`, `required_signatures`, `pull_request`, `update` all accepted on this repo.

- [x] Document and pin **whitelisted git identities** — [`config/commit-email-allowlist`](config/commit-email-allowlist)
- [x] CI guard on PRs/push — [`.github/workflows/policy.yml`](.github/workflows/policy.yml) runs `identity:check` + `commitlint:range` (authoritative; local ad-hoc only)
- [ ] GitHub ruleset: reject pushes whose commit author/committer is outside the whitelist (email-pattern rules on `main` / default)
- [ ] Block **merging PRs** that contain any commit outside the whitelist (require `Policy` status + optional ruleset patterns)
- [ ] Restrict direct pushes to protected refs (`update` / PR-required + bypass list for org admins)
- [ ] Optional: require verified signed commits on protected branches (`required_signatures`)

## CI / builds

- [ ] PR builds on standard GitHub-hosted runners (or dispatch from PR head) so work does not depend on local monorepo compiles
- [ ] Shared Actions cache across branches/PRs (deps + target; document what actually hits)
- [ ] Keep workflow `UPSTREAM_SHA` / release pins aligned after each upstream sync
- [ ] Stay on free GitHub-hosted runner SKUs (no large/self-hosted unless approved)

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
- [ ] Keep this ledger current

## Upstream

- [ ] Periodic sync from `upstream` (FF when possible; else sync PR + CI — see `AGENTS.md`)
