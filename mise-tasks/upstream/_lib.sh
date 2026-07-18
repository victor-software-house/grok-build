# Shared helpers for upstream:status / upstream:sync.
# shellcheck shell=bash

# Branch prefix + labels identify automation-owned sync PRs (deterministic).
UPSTREAM_SYNC_BRANCH_PREFIX="sync/upstream-"
UPSTREAM_SYNC_LABEL="sync:upstream"
UPSTREAM_SYNC_LABEL_CLEAN="sync:clean"
UPSTREAM_SYNC_LABEL_CONFLICT="sync:conflict"
UPSTREAM_FALLBACK_URL="https://github.com/xai-org/grok-build.git"

upstream_short() {
  local sha=$1
  printf '%s' "${sha:0:12}"
}

upstream_branch_for() {
  local sha=$1
  printf '%s%s' "$UPSTREAM_SYNC_BRANCH_PREFIX" "$(upstream_short "$sha")"
}

upstream_parse_branch_sha_prefix() {
  # Print the short suffix after sync/upstream- (may be 7–40 hex).
  local ref=$1
  local pfx=$UPSTREAM_SYNC_BRANCH_PREFIX
  case "$ref" in
    "$pfx"*) printf '%s' "${ref#"$pfx"}" ;;
    *) return 1 ;;
  esac
}

ensure_upstream_remote() {
  if git remote get-url upstream >/dev/null 2>&1; then
    return 0
  fi
  git remote add upstream "$UPSTREAM_FALLBACK_URL"
}

fetch_origin_upstream_main() {
  ensure_upstream_remote
  git fetch origin main --quiet
  git fetch upstream main --quiet
}

origin_main_sha() {
  git rev-parse "origin/main"
}

upstream_main_sha() {
  git rev-parse "upstream/main"
}

is_ancestor() {
  # $1 ancestor of $2?
  git merge-base --is-ancestor "$1" "$2"
}

# List open sync PRs as lines: number|headRefName|url|labels_csv
# labels_csv is comma-separated label names.
list_open_sync_prs() {
  local repo=$1
  gh pr list -R "$repo" --state open --label "$UPSTREAM_SYNC_LABEL" --limit 50 \
    --json number,headRefName,url,labels \
    --jq '.[] | "\(.number)|\(.headRefName)|\(.url)|\([.labels[].name] | join(","))"'
}

pr_has_label() {
  local labels_csv=$1
  local want=$2
  [[ ",${labels_csv}," == *",${want},"* ]]
}

ensure_sync_labels() {
  local repo=$1
  # Best-effort create; ignore if exists.
  gh label create "$UPSTREAM_SYNC_LABEL" -R "$repo" --color "0E8A16" --description "Upstream monorepo sync PR" 2>/dev/null || true
  gh label create "$UPSTREAM_SYNC_LABEL_CLEAN" -R "$repo" --color "1D76DB" --description "Sync merge is clean" 2>/dev/null || true
  gh label create "$UPSTREAM_SYNC_LABEL_CONFLICT" -R "$repo" --color "B60205" --description "Sync merge has conflicts — thorough review required" 2>/dev/null || true
}

source_rev_at() {
  local ref=$1
  git show "${ref}:SOURCE_REV" 2>/dev/null | tr -d '[:space:]' || true
}

# Build a merge of $upstream_tip onto $base in a throwaway worktree.
# Sets globals: BUILD_DIR BUILD_BRANCH BUILD_CLEAN BUILD_HEAD
# BUILD_CLEAN is "true" or "false".
build_sync_merge() {
  local base=$1
  local upstream_tip=$2
  local short
  short=$(upstream_short "$upstream_tip")
  BUILD_BRANCH=$(upstream_branch_for "$upstream_tip")
  BUILD_DIR=$(mktemp -d "${TMPDIR:-/tmp}/upstream-sync.XXXXXX")
  BUILD_CLEAN=true

  # Avoid lefthook post-checkout in the throwaway worktree.
  LEFTHOOK=0 git worktree add -B "$BUILD_BRANCH" "$BUILD_DIR" "$base" --quiet

  # Identity for the merge commit (CI bot when in Actions; else local user).
  if [[ "${GITHUB_ACTIONS:-}" == "true" || -n "${CI:-}" ]]; then
    git -C "$BUILD_DIR" config user.email "41898282+github-actions[bot]@users.noreply.github.com"
    git -C "$BUILD_DIR" config user.name "github-actions[bot]"
  fi

  local msg
  msg="chore(sync): upstream main @ ${short}"

  if git -C "$BUILD_DIR" merge --no-ff --no-edit -m "$msg" "$upstream_tip" >/dev/null 2>&1; then
    BUILD_CLEAN=true
  else
    BUILD_CLEAN=false
    # Stage conflict markers and complete the merge so the PR is reviewable.
    git -C "$BUILD_DIR" add -A
    if ! git -C "$BUILD_DIR" diff --cached --quiet 2>/dev/null; then
      git -C "$BUILD_DIR" commit --no-edit -m "${msg} (conflicts)" >/dev/null 2>&1 \
        || git -C "$BUILD_DIR" commit -m "${msg} (conflicts)" >/dev/null
    else
      # Unexpected empty conflict — abort hard.
      echo "upstream:sync: merge failed with no staged changes" >&2
      return 1
    fi
  fi

  BUILD_HEAD=$(git -C "$BUILD_DIR" rev-parse HEAD)
}

cleanup_build_worktree() {
  if [[ -n "${BUILD_DIR:-}" && -d "${BUILD_DIR:-}" ]]; then
    git worktree remove --force "$BUILD_DIR" 2>/dev/null || true
    rm -rf "$BUILD_DIR" 2>/dev/null || true
  fi
  BUILD_DIR=""
  # Drop throwaway local branch name (remote tip is pushed by SHA).
  if [[ -n "${BUILD_BRANCH:-}" ]]; then
    git branch -D "$BUILD_BRANCH" >/dev/null 2>&1 || true
  fi
}

push_sync_branch() {
  local branch=$1
  local head=$2
  # Always lease-protected force: safe on first push and rewrites of this tip branch.
  git push --force-with-lease origin "${head}:refs/heads/${branch}"
}

find_open_pr_for_branch() {
  local repo=$1
  local branch=$2
  local line num head
  while IFS= read -r line; do
    [[ -z "$line" ]] && continue
    num=${line%%|*}
    rest=${line#*|}
    head=${rest%%|*}
    if [[ "$head" == "$branch" ]]; then
      printf '%s' "$num"
      return 0
    fi
  done < <(list_open_sync_prs "$repo")
  return 1
}

close_pr_with_comment() {
  local repo=$1
  local number=$2
  local comment=$3
  gh pr close "$number" -R "$repo" --comment "$comment" >/dev/null
}

pr_body_for() {
  local base=$1
  local tip=$2
  local clean=$3
  local short
  short=$(upstream_short "$tip")
  local base_sr tip_sr
  base_sr=$(source_rev_at "$base")
  tip_sr=$(source_rev_at "$tip")

  local status_line review_line
  if [[ "$clean" == "true" ]]; then
    status_line="**Merge status:** clean (no conflicts onto \`origin/main\` at automation time)."
    review_line="Review the product delta, then merge with \`mise run pr:merge -- <N>\` when ready. **Do not auto-merge.**"
  else
    status_line="**Merge status:** **CONFLICTS** — conflict markers are committed in the branch so they are visible in the diff."
    review_line="**Thorough review required.** Resolve conflicts on this branch (or locally), push, re-check CI, then merge with \`mise run pr:merge -- <N>\`. Automation may replace this dirty PR if a newer conflicted upstream tip lands."
  fi

  cat <<EOF
## Upstream sync

${status_line}

| | SHA |
|:--|:--|
| \`origin/main\` (base) | \`${base}\` |
| \`upstream/main\` (tip) | \`${tip}\` (\`${short}\`) |
| \`SOURCE_REV\` on base | \`${base_sr:-?}\` |
| \`SOURCE_REV\` on tip | \`${tip_sr:-?}\` |

### Policy

- One PR per upstream tip; at most **two** open sync PRs (latest **clean** + latest **dirty**).
- A newer **clean** tip closes all other open sync PRs.
- A newer **dirty** tip replaces only the previous dirty PR (clean slot kept).
- **Never auto-merge** — operator review is the gate.

${review_line}

Upstream-only commits:

\`\`\`
$(git log --oneline "${base}..${tip}" | head -30)
\`\`\`
EOF
}

ensure_pr_for_branch() {
  local repo=$1
  local branch=$2
  local base_sha=$3
  local tip=$4
  local clean=$5
  local title body labels existing
  local short
  short=$(upstream_short "$tip")
  title="chore(sync): upstream main @ ${short}"
  if [[ "$clean" != "true" ]]; then
    title="${title} (conflicts)"
  fi
  body=$(pr_body_for "$base_sha" "$tip" "$clean")
  labels=("$UPSTREAM_SYNC_LABEL")
  if [[ "$clean" == "true" ]]; then
    labels+=("$UPSTREAM_SYNC_LABEL_CLEAN")
  else
    labels+=("$UPSTREAM_SYNC_LABEL_CONFLICT")
  fi

  existing=$(find_open_pr_for_branch "$repo" "$branch" || true)
  if [[ -n "$existing" ]]; then
    gh pr edit "$existing" -R "$repo" --title "$title" --body "$body" >/dev/null
    gh pr edit "$existing" -R "$repo" --remove-label "$UPSTREAM_SYNC_LABEL_CLEAN" 2>/dev/null || true
    gh pr edit "$existing" -R "$repo" --remove-label "$UPSTREAM_SYNC_LABEL_CONFLICT" 2>/dev/null || true
    if [[ "$clean" == "true" ]]; then
      gh pr edit "$existing" -R "$repo" --add-label "$UPSTREAM_SYNC_LABEL_CLEAN" >/dev/null
    else
      gh pr edit "$existing" -R "$repo" --add-label "$UPSTREAM_SYNC_LABEL_CONFLICT" >/dev/null
    fi
    printf '%s' "$existing"
    return 0
  fi

  local label_args=()
  local l url num
  for l in "${labels[@]}"; do
    label_args+=(--label "$l")
  done
  url=$(gh pr create -R "$repo" --base main --head "$branch" --title "$title" --body "$body" "${label_args[@]}")
  num=$(gh pr view "$url" -R "$repo" --json number --jq .number)
  printf '%s' "$num"
}

# Resolve a branch suffix (short sha) to a full commit reachable from upstream/main or origin.
resolve_tip_from_branch() {
  local branch=$1
  local suffix
  suffix=$(upstream_parse_branch_sha_prefix "$branch") || return 1
  # Prefer exact match under upstream/main history.
  if git rev-parse --verify "${suffix}^{commit}" >/dev/null 2>&1; then
    git rev-parse "${suffix}^{commit}"
    return 0
  fi
  # Ambiguous / truncated: find commit on upstream/main that starts with suffix.
  local c
  c=$(git rev-list upstream/main --not origin/main 2>/dev/null | grep -E "^${suffix}" | head -1 || true)
  if [[ -n "$c" ]]; then
    printf '%s' "$c"
    return 0
  fi
  c=$(git rev-list upstream/main | grep -E "^${suffix}" | head -1 || true)
  if [[ -n "$c" ]]; then
    printf '%s' "$c"
    return 0
  fi
  return 1
}

should_apply() {
  # Default: dry-run. Apply when --apply, or when CI/GITHUB_ACTIONS and not --dry-run.
  if [[ "${FORCE_DRY_RUN:-false}" == "true" ]]; then
    return 1
  fi
  if [[ "${FORCE_APPLY:-false}" == "true" ]]; then
    return 0
  fi
  if [[ "${GITHUB_ACTIONS:-}" == "true" || -n "${CI:-}" ]]; then
    return 0
  fi
  return 1
}
