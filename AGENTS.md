# Agent guide — grok-build

This is the **Victor Software House** fork of SpaceXAI’s [Grok Build](https://x.ai/cli) CLI/TUI (Rust monorepo).\
It tells agents how to work **on this repository**.\
It is *not* the product `AGENTS.md` that Grok may load from other projects.

| Audience | Start here |
|:--|:--|
| Humans | [`README.md`](README.md) · [`CONTRIBUTING.md`](CONTRIBUTING.md) · [`SECURITY.md`](SECURITY.md) |
| Agents | *this file* · [`CLAUDE.md`](CLAUDE.md) (imports it) · [`TODO.md`](TODO.md) |

### Writing

Keep it short and specific to this fork.\
Do not restate industry defaults that agents already know.

### Markdown

A single newline collapses into a space.\
End **related** sentences with a trailing `\` so they stack on consecutive lines.\
Use a blank line when the next sentence is a new topic or paragraph.

Mix syntax for scanability: **bold**, *italics*, `code`, [links](README.md), lists, tables, and `>` callouts.\
Do not write one dense paragraph wall.

---

## Public tree

> Commit **technical** content only. This repo is public.

- No secrets, tokens, or credentials
- No private hostnames, client/employer operational detail, or personal contact data
- Do not open contributor CLA-style flows — see [`CONTRIBUTING.md`](CONTRIBUTING.md)
- Security reports go to [`SECURITY.md`](SECURITY.md)

---

## Workflow (mandatory)

> Do **not** leave uncommitted work on `main`.

1. **Branch** from up-to-date `main`\
   Names: `feat/…`, `fix/…`, `docs/…`, `ci/…`, or `chore/…`.
2. **Commit** with [Conventional Commits](https://www.conventionalcommits.org/) as you go.\
   Keep commits small and reviewable.
3. **Push** the branch to `origin`:\
   `git push -u origin HEAD`
4. **Open a PR** when the work is ready.\
   Post the URL and a short summary, then **stop**.
5. **Merge only** after explicit approval in the conversation:\
   [`mise run pr:merge -- <N>`](mise-tasks/pr/merge)
   - Always deletes the PR head branch
   - **One** commit on the PR → *squash* merge
   - **Two or more** commits → *merge commit*
   - Overrides only if asked: `--squash` or `--merge-commit`
6. **After merge:** [`mise run main:sync`](mise-tasks/main/sync)\
   Never delete or rewrite `archive/**` branches.

Leaving a dirty `main` or an unpushed branch is a **process failure**.

---

## Git

### Identity

Author and committer emails must match [`config/commit-email-allowlist`](config/commit-email-allowlist).

| Where | How |
|:--|:--|
| **CI (authoritative)** | [`.github/workflows/policy.yml`](.github/workflows/policy.yml) |
| Local (ad-hoc) | [`mise run identity:check`](mise-tasks/identity/check) |

### Remotes and history

| Remote | Points at |
|:--|:--|
| `origin` | this fork (`victor-software-house/grok-build`) |
| `upstream` | [`xai-org/grok-build`](https://github.com/xai-org/grok-build) |

- `archive/**` is permanent on `origin` — never delete or force-push those branches
- Do not rewrite published history unless the operator asks and branch rules allow it

### Upstream sync

Fast-forward `main` when you can.\
Otherwise open a sync branch, PR, and let CI run.

After a sync, confirm [`SOURCE_REV`](SOURCE_REV) matches the intended upstream snapshot.

Do not invent a parallel history of the upstream tree.\
Do not force-push `main` unless the operator allows it and org rules permit it.

```sh
git fetch upstream origin
git log --oneline origin/main..upstream/main
```

---

## Build / CI

Do not compile this whole monorepo on the laptop by default.\
Use **GitHub-hosted free-tier runners** instead.

| On GitHub | Local (only if the operator asks) |
|:--|:--|
| [Policy](.github/workflows/policy.yml): identity + commitlint on PR / push to `main` | [`mise run check\|clippy\|test -- <crate>`](mise.toml) |
| Release packaging: [`ci:dispatch`](mise-tasks/ci/dispatch) / [`ci:watch`](mise-tasks/ci/watch) | No workspace `cargo build --release` without approval |
| Workflow definition: [`build-macos-arm64.yml`](.github/workflows/build-macos-arm64.yml) | Install helper: [`scripts/install-github-release.sh`](scripts/install-github-release.sh) |

---

## Tooling

- Interactive shells: [`.envrc`](.envrc) → `use_mise_env` (direnv)
- **Never** `mise activate` — direnv owns PATH injection
- Agents / CI / scripts: `mise run …` or `mise x -- …` via [`mise.toml`](mise.toml)
- Rust: [`rust-toolchain.toml`](rust-toolchain.toml) + rustup (not mise)
- Git hooks: thin [`lefthook.yml`](lefthook.yml) → [`mise-tasks/`](mise-tasks/)
- Local hooks: `commit-msg` only
- Identity: CI only — not lefthook

### Common tasks

| Task | What it does |
|:--|:--|
| [`fmt`](mise.toml) / `fmt:check` | Format with `cargo fmt --all` (write / check) |
| `check\|clippy\|test -- <crate>` | Crate-scoped cargo (not the full workspace) |
| [`commitlint:msg`](mise-tasks/commitlint/msg) / [`commitlint:range`](mise-tasks/commitlint/range) | Validate Conventional Commit subjects |
| [`identity:check`](mise-tasks/identity/check) | Check author/committer emails against the allowlist |
| [`pr:merge -- <N>`](mise-tasks/pr/merge) | Merge a PR using the defaults under [Workflow](#workflow-mandatory) |
| [`main:sync`](mise-tasks/main/sync) | After merge: update `main` and drop locals whose remote is gone |
| [`ci:dispatch`](mise-tasks/ci/dispatch) / [`ci:watch`](mise-tasks/ci/watch) | Dispatch / watch the macOS release workflow |
| `workflows:lint` | Lint workflows with actionlint |
| `hooks:install` / [`worktree:setup`](mise-tasks/worktree/setup) | Install lefthook; set up a linked worktree |

List every task: `mise tasks`.

---

## Layout

| Area | Path |
|:--|:--|
| TUI / CLI | [`xai-grok-pager-bin`](crates/codegen/xai-grok-pager-bin) · [`xai-grok-pager`](crates/codegen/xai-grok-pager) |
| Agent / shell | [`xai-grok-shell`](crates/codegen/xai-grok-shell) · [`xai-grok-agent`](crates/codegen/xai-grok-agent) |
| Tools | [`xai-tool-types`](crates/common/xai-tool-types) · [`xai-tool-runtime`](crates/common/xai-tool-runtime) · [`xai-tool-protocol`](crates/common/xai-tool-protocol) |
| Workspace / config | [`xai-grok-workspace`](crates/codegen/xai-grok-workspace) · [`xai-grok-config`](crates/codegen/xai-grok-config) · [`xai-grok-config-types`](crates/codegen/xai-grok-config-types) |
| Fork CI / install | [`.github/workflows/`](.github/workflows/) · [`scripts/install-github-release.sh`](scripts/install-github-release.sh) |

---

## Remote tool execution (intent)

Run the same tool handlers as local work through a **persistent daemon** over RPC\
([`xai-tool-protocol`](crates/common/xai-tool-protocol) / [`xai-tool-runtime`](crates/common/xai-tool-runtime)),\
not by wrapping each call in SSH one-liners.

Prove loopback first, then remote.

Details and open work: [`TODO.md`](TODO.md).

---

## When stuck

1. Read crate docs and the [README development section](README.md#development).
2. Prefer [GitHub Actions](.github/workflows/) over a local monorepo build.
3. Extend the existing protocol and runtime crates before inventing a new system.
