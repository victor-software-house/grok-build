# Todo ledger

Fork-local work tracking. Technical only (no secrets, no client/personal data).


## Repo policy / branch protection

- [ ] Document and pin **whitelisted git identities** (author/committer) for this fork (org email / noreply forms only)
- [ ] GitHub ruleset: reject pushes whose commit author/committer is outside the whitelist (email-pattern rules)
- [ ] Block **merging PRs** that contain any commit outside the whitelist (status check or ruleset — no merge with foreign identities)
- [ ] Restrict direct pushes to protected refs to whitelisted GitHub actors (ruleset restrict-updates + bypass list)
- [ ] Optional: require verified signed commits on protected branches
- [ ] CI guard on PRs: fail if any commit author/committer email is not in the allowlist

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

## Docs

- [x] Root `AGENTS.md` + `CLAUDE.md`
- [x] Prefer GitHub-hosted CI over local monorepo builds
- [x] Conventional Commits + type-prefixed branches (brief)
- [x] Explicit operator approval before merge
- [x] Merge method default + archive/** retention
- [ ] Keep this ledger current

## Upstream

- [ ] Periodic sync from `upstream` (FF when possible; else sync PR + CI — see `AGENTS.md`)
