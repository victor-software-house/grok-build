# Shared helpers for upstream:status / upstream:sync.
# shellcheck shell=bash

# Resolve repo root once; pull in shared fork helpers.
UPSTREAM_LIB_ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
# shellcheck source=../_lib/gh-origin.sh
source "${UPSTREAM_LIB_ROOT}/mise-tasks/_lib/gh-origin.sh"
# shellcheck source=../_lib/require-cmd.sh
source "${UPSTREAM_LIB_ROOT}/mise-tasks/_lib/require-cmd.sh"

# Branch prefix + labels identify automation-owned sync PRs (deterministic).
UPSTREAM_SYNC_BRANCH_PREFIX="sync/upstream-"
UPSTREAM_SYNC_LABEL="sync:upstream"
UPSTREAM_SYNC_LABEL_CLEAN="sync:clean"
UPSTREAM_SYNC_LABEL_CONFLICT="sync:conflict"
UPSTREAM_FALLBACK_URL="https://github.com/xai-org/grok-build.git"

# Set by upstream_bootstrap: UPSTREAM_TASK REPO
UPSTREAM_TASK=""
REPO=""

upstream_bootstrap() {
  UPSTREAM_TASK=$1
  require_cmd jq "$UPSTREAM_TASK"
  REPO=$(origin_name_with_owner)
  ensure_gh_default_origin
}

upstream_short() {
  printf '%s' "${1:0:12}"
}

upstream_branch_for() {
  printf '%s%s' "$UPSTREAM_SYNC_BRANCH_PREFIX" "$(upstream_short "$1")"
}

upstream_parse_branch_sha_prefix() {
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

origin_main_sha() { git rev-parse origin/main; }
upstream_main_sha() { git rev-parse upstream/main; }

is_ancestor() {
  # $1 ancestor of $2?
  git merge-base --is-ancestor "$1" "$2"
}

source_rev_at() {
  git show "${1}:SOURCE_REV" 2>/dev/null | tr -d '[:space:]' || true
}

# --- Open sync PR records: number|headRefName|url|labels_csv ---

list_open_sync_prs() {
  # REST only — avoid `gh pr` GraphQL (touches fork parent; xai-org IP allowlist breaks Actions).
  local repo=${1:-$REPO}
  gh api "repos/${repo}/pulls?state=open&per_page=50" \
    | jq -r --arg lab "$UPSTREAM_SYNC_LABEL" '
        .[]
        | select([.labels[].name] | index($lab))
        | "\(.number)|\(.head.ref)|\(.html_url)|\([.labels[].name] | join(","))"
      '
}

# Parse one list_open_sync_prs line → PR_NUM PR_HEAD PR_URL PR_LABELS
parse_sync_pr_line() {
  local line=$1
  PR_NUM=${line%%|*}
  local rest=${line#*|}
  PR_HEAD=${rest%%|*}
  rest=${rest#*|}
  PR_URL=${rest%%|*}
  PR_LABELS=${rest#*|}
}

pr_has_label() {
  local labels_csv=$1
  local want=$2
  [[ ",${labels_csv}," == *",${want},"* ]]
}

pr_kind() {
  # echo clean|dirty|?
  local labels=$1
  if pr_has_label "$labels" "$UPSTREAM_SYNC_LABEL_CONFLICT"; then
    printf dirty
  elif pr_has_label "$labels" "$UPSTREAM_SYNC_LABEL_CLEAN"; then
    printf clean
  else
    printf '?'
  fi
}

ensure_sync_labels() {
  local repo=${1:-$REPO}
  _ensure_label "$repo" "$UPSTREAM_SYNC_LABEL" "0E8A16" "Upstream monorepo sync PR"
  _ensure_label "$repo" "$UPSTREAM_SYNC_LABEL_CLEAN" "1D76DB" "Sync merge is clean"
  _ensure_label "$repo" "$UPSTREAM_SYNC_LABEL_CONFLICT" "B60205" "Sync merge has conflicts — thorough review required"
}

_ensure_label() {
  local repo=$1 name=$2 color=$3 desc=$4
  gh api -X POST "repos/${repo}/labels" \
    -f name="$name" -f color="$color" -f description="$desc" >/dev/null 2>&1 \
    || gh api -X PATCH "repos/${repo}/labels/$(printf %s "$name" | jq -sRr @uri)" \
      -f color="$color" -f description="$desc" >/dev/null 2>&1 \
    || true
}

close_pr_with_comment() {
  local number=$1
  local comment=$2
  local repo=${3:-$REPO}
  gh api "repos/${repo}/issues/${number}/comments" -f body="$comment" >/dev/null
  gh api -X PATCH "repos/${repo}/pulls/${number}" -f state=closed >/dev/null
}

# Close open sync PRs matching filter (all|dirty|clean), optionally keeping one number.
# close_sync_prs_except KEEP_NUM FILTER COMMENT_TEMPLATE
# COMMENT may contain %s for the closed PR number context; keep is $1 available as $keep in message — pass full comment.
close_sync_prs_except() {
  local keep=$1
  local filter=${2:-all}
  local comment=$3
  local line kind

  while IFS= read -r line; do
    [[ -z "$line" ]] && continue
    parse_sync_pr_line "$line"
    [[ "$PR_NUM" == "$keep" ]] && continue

    kind=$(pr_kind "$PR_LABELS")
    case "$filter" in
      dirty) [[ "$kind" == dirty ]] || continue ;;
      clean) [[ "$kind" == clean ]] || continue ;;
      all) ;;
      *)
        printf '%s: unknown close filter %s\n' "$UPSTREAM_TASK" "$filter" >&2
        return 2
        ;;
    esac

    echo "  closing #${PR_NUM} (${PR_HEAD}) filter=${filter}"
    close_pr_with_comment "$PR_NUM" "$comment"
  done < <(list_open_sync_prs || true)
}

close_all_sync_prs() {
  local comment=$1
  local line
  while IFS= read -r line; do
    [[ -z "$line" ]] && continue
    parse_sync_pr_line "$line"
    echo "  closing #${PR_NUM} (${PR_HEAD})"
    close_pr_with_comment "$PR_NUM" "$comment"
  done < <(list_open_sync_prs || true)
}

find_open_pr_for_branch() {
  local branch=$1
  local line
  while IFS= read -r line; do
    [[ -z "$line" ]] && continue
    parse_sync_pr_line "$line"
    if [[ "$PR_HEAD" == "$branch" ]]; then
      printf '%s' "$PR_NUM"
      return 0
    fi
  done < <(list_open_sync_prs || true)
  return 1
}

# Print open sync PRs; sets OPEN_COUNT CLEAN_COUNT DIRTY_COUNT.
print_open_sync_prs() {
  local indent=${1:-    }
  OPEN_COUNT=0
  CLEAN_COUNT=0
  DIRTY_COUNT=0
  local line kind

  while IFS= read -r line; do
    [[ -z "$line" ]] && continue
    parse_sync_pr_line "$line"
    OPEN_COUNT=$((OPEN_COUNT + 1))
    kind=$(pr_kind "$PR_LABELS")
    case "$kind" in
      dirty) DIRTY_COUNT=$((DIRTY_COUNT + 1)) ;;
      clean) CLEAN_COUNT=$((CLEAN_COUNT + 1)) ;;
    esac
    printf '%s#%s [%s] %s\n' "$indent" "$PR_NUM" "$kind" "$PR_HEAD"
    printf '%s     %s\n' "$indent" "$PR_URL"
  done < <(list_open_sync_prs || true)

  if [[ "$OPEN_COUNT" -eq 0 ]]; then
    printf '%s(none)\n' "$indent"
  fi
}

# Build merge of $upstream_tip onto $base in a throwaway worktree.
# Sets: BUILD_DIR BUILD_BRANCH BUILD_CLEAN BUILD_HEAD
build_sync_merge() {
  local base=$1
  local upstream_tip=$2
  local short msg
  short=$(upstream_short "$upstream_tip")
  BUILD_BRANCH=$(upstream_branch_for "$upstream_tip")
  BUILD_DIR=$(mktemp -d "${TMPDIR:-/tmp}/upstream-sync.XXXXXX")
  BUILD_CLEAN=true

  LEFTHOOK=0 git worktree add -B "$BUILD_BRANCH" "$BUILD_DIR" "$base" --quiet

  if [[ "${GITHUB_ACTIONS:-}" == "true" || -n "${CI:-}" ]]; then
    git -C "$BUILD_DIR" config user.email "41898282+github-actions[bot]@users.noreply.github.com"
    git -C "$BUILD_DIR" config user.name "github-actions[bot]"
  fi

  msg="chore(sync): upstream main @ ${short}"

  if git -C "$BUILD_DIR" merge --no-ff --no-edit -m "$msg" "$upstream_tip" >/dev/null 2>&1; then
    BUILD_CLEAN=true
  else
    BUILD_CLEAN=false
    git -C "$BUILD_DIR" add -A
    if ! git -C "$BUILD_DIR" diff --cached --quiet 2>/dev/null; then
      git -C "$BUILD_DIR" commit --no-edit -m "${msg} (conflicts)" >/dev/null 2>&1 \
        || git -C "$BUILD_DIR" commit -m "${msg} (conflicts)" >/dev/null
    else
      echo "${UPSTREAM_TASK:-upstream}: merge failed with no staged changes" >&2
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
  if [[ -n "${BUILD_BRANCH:-}" ]]; then
    git branch -D "$BUILD_BRANCH" >/dev/null 2>&1 || true
  fi
}

push_sync_branch() {
  local branch=$1
  local head=$2
  git push --force-with-lease origin "${head}:refs/heads/${branch}"
}

pr_body_for() {
  local base=$1
  local tip=$2
  local clean=$3
  local short base_sr tip_sr status_line review_line
  short=$(upstream_short "$tip")
  base_sr=$(source_rev_at "$base")
  tip_sr=$(source_rev_at "$tip")

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

_set_pr_labels() {
  local repo=$1 number=$2 clean=$3
  # Replace clean/conflict; keep sync:upstream.
  local labels
  if [[ "$clean" == "true" ]]; then
    labels=$(jq -nc --arg a "$UPSTREAM_SYNC_LABEL" --arg b "$UPSTREAM_SYNC_LABEL_CLEAN" '{labels:[$a,$b]}')
  else
    labels=$(jq -nc --arg a "$UPSTREAM_SYNC_LABEL" --arg b "$UPSTREAM_SYNC_LABEL_CONFLICT" '{labels:[$a,$b]}')
  fi
  # Remove both kind labels then set the set (PUT replaces all issue labels — keep only ours).
  gh api -X PUT "repos/${repo}/issues/${number}/labels" --input - <<<"$labels" >/dev/null
}

ensure_pr_for_branch() {
  local branch=$1
  local base_sha=$2
  local tip=$3
  local clean=$4
  local repo=${5:-$REPO}
  local title body short existing num

  short=$(upstream_short "$tip")
  title="chore(sync): upstream main @ ${short}"
  if [[ "$clean" != "true" ]]; then
    title="${title} (conflicts)"
  fi
  body=$(pr_body_for "$base_sha" "$tip" "$clean")

  existing=$(find_open_pr_for_branch "$branch" || true)
  if [[ -n "$existing" ]]; then
    gh api -X PATCH "repos/${repo}/pulls/${existing}" -f title="$title" -f body="$body" >/dev/null
    _set_pr_labels "$repo" "$existing" "$clean"
    printf '%s' "$existing"
    return 0
  fi

  num=$(gh api "repos/${repo}/pulls" \
    -f title="$title" \
    -f head="$branch" \
    -f base=main \
    -f body="$body" \
    --jq .number) || {
    echo "${UPSTREAM_TASK:-upstream}: REST create PR failed for head=${branch}" >&2
    return 1
  }
  _set_pr_labels "$repo" "$num" "$clean"
  printf '%s' "$num"
}

resolve_tip_from_branch() {
  local branch=$1
  local suffix c
  suffix=$(upstream_parse_branch_sha_prefix "$branch") || return 1
  if git rev-parse --verify "${suffix}^{commit}" >/dev/null 2>&1; then
    git rev-parse "${suffix}^{commit}"
    return 0
  fi
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

merge_probe_clean() {
  # true if base+tip merge is clean (no worktree).
  git merge-tree --write-tree "$1" "$2" >/dev/null 2>&1
}

# --- High-level status / sync orchestration ---

# Sets BASE UP SHORT BRANCH BASE_SR UP_SR after fetch.
load_upstream_tips() {
  fetch_origin_upstream_main
  BASE=$(origin_main_sha)
  UP=$(upstream_main_sha)
  SHORT=$(upstream_short "$UP")
  BRANCH=$(upstream_branch_for "$UP")
  BASE_SR=$(source_rev_at "$BASE")
  UP_SR=$(source_rev_at "$UP")
}

print_tip_header() {
  echo "  origin:        ${REPO}"
  echo "  origin/main:   ${BASE}"
  echo "  upstream/main: ${UP} (${SHORT})"
  echo "  SOURCE_REV base: ${BASE_SR:-?}"
  echo "  SOURCE_REV tip:  ${UP_SR:-?}"
}

# Refresh a clean-slot PR onto current BASE; close if no longer clean.
refresh_clean_slot_pr() {
  local num=$1
  local head=$2
  local other_tip other_clean other_head other_branch

  other_tip=$(resolve_tip_from_branch "$head" || true)
  if [[ -z "${other_tip:-}" ]]; then
    echo "  warn: cannot resolve tip for ${head}; closing #${num}"
    close_pr_with_comment "$num" \
      "Closed by upstream:sync: could not resolve upstream tip from branch name \`${head}\`."
    return 0
  fi

  trap cleanup_build_worktree EXIT
  build_sync_merge "$BASE" "$other_tip"
  other_clean=$BUILD_CLEAN
  other_head=$BUILD_HEAD
  other_branch=$BUILD_BRANCH

  if [[ "$other_clean" != "true" ]]; then
    echo "  clean slot #${num} no longer clean on current main — closing"
    close_pr_with_comment "$num" \
      "Closed by upstream:sync: former clean tip is no longer conflict-free on current \`main\`."
    cleanup_build_worktree
    trap - EXIT
    return 0
  fi

  push_sync_branch "$other_branch" "$other_head"
  ensure_pr_for_branch "$other_branch" "$BASE" "$other_tip" true >/dev/null
  echo "  refreshed clean slot #${num} (${other_branch})"
  cleanup_build_worktree
  trap - EXIT
}

# After a dirty tip PR is ensured: refresh clean slots; cap at one clean.
maintain_clean_slots_after_dirty() {
  local tip_pr=$1
  local line num head kind cleans=() sorted keep_clean

  while IFS= read -r line; do
    [[ -z "$line" ]] && continue
    parse_sync_pr_line "$line"
    [[ "$PR_NUM" == "$tip_pr" ]] && continue
    [[ "$(pr_kind "$PR_LABELS")" == clean ]] || continue
    refresh_clean_slot_pr "$PR_NUM" "$PR_HEAD"
  done < <(list_open_sync_prs || true)

  while IFS= read -r line; do
    [[ -z "$line" ]] && continue
    parse_sync_pr_line "$line"
    [[ "$PR_NUM" == "$tip_pr" ]] && continue
    [[ "$(pr_kind "$PR_LABELS")" == clean ]] || continue
    cleans+=("$PR_NUM")
  done < <(list_open_sync_prs || true)

  if [[ ${#cleans[@]} -gt 1 ]]; then
    mapfile -t sorted < <(printf '%s\n' "${cleans[@]}" | sort -nr)
    keep_clean=${sorted[0]}
    for num in "${sorted[@]:1}"; do
      echo "  closing extra clean #${num} (keeping #${keep_clean})"
      close_pr_with_comment "$num" \
        "Closed by upstream:sync: only one clean slot allowed; kept #${keep_clean}."
    done
  fi
}

# Apply path for current UP onto BASE (tips already loaded).
# Uses tip merge already built: TIP_CLEAN TIP_HEAD TIP_BRANCH
apply_tip_sync() {
  local tip_pr short_msg

  ensure_sync_labels
  push_sync_branch "$TIP_BRANCH" "$TIP_HEAD"
  tip_pr=$(ensure_pr_for_branch "$TIP_BRANCH" "$BASE" "$UP" "$TIP_CLEAN") || return 1
  if [[ -z "${tip_pr}" || "$tip_pr" == "?" ]]; then
    echo "${UPSTREAM_TASK:-upstream}: failed to open/update PR for ${TIP_BRANCH}" >&2
    return 1
  fi
  echo "  tip PR: #${tip_pr} (${TIP_BRANCH})"

  short_msg="superseded by #${tip_pr} (\`upstream/main\` @ \`${SHORT}\`, clean=${TIP_CLEAN})"

  if [[ "$TIP_CLEAN" == "true" ]]; then
    close_sync_prs_except "$tip_pr" all \
      "Closed by upstream:sync: ${short_msg}."
  else
    close_sync_prs_except "$tip_pr" dirty \
      "Closed by upstream:sync: ${short_msg}."
    maintain_clean_slots_after_dirty "$tip_pr"
  fi
}

run_upstream_status() {
  upstream_bootstrap "upstream:status"
  load_upstream_tips

  echo "upstream:status"
  print_tip_header

  if is_ancestor "$UP" "$BASE"; then
    echo "  state: already integrated (upstream/main ⊆ origin/main)"
  else
    echo "  divergence: origin ahead=$(git rev-list --count "${UP}..${BASE}") behind=$(git rev-list --count "${BASE}..${UP}") (vs upstream tip)"
    echo "  upstream-only:"
    git log --oneline "${BASE}..${UP}" | sed 's/^/    /' | head -20
  fi

  echo "  open sync PRs (label ${UPSTREAM_SYNC_LABEL}):"
  print_open_sync_prs "    "

  if [[ "$DIRTY_COUNT" -gt 0 ]]; then
    echo
    echo "  WARNING: ${DIRTY_COUNT} conflicted sync PR(s) open."
    echo "  Thorough review required before merge — conflict markers are intentional."
    echo "  Merge only via mise run pr:merge after resolving/reviewing."
  fi

  if [[ "$OPEN_COUNT" -gt 2 ]]; then
    echo
    echo "  WARNING: ${OPEN_COUNT} open sync PRs (policy cap is 2). Run: mise run upstream:sync -- --apply"
  fi

  if ! is_ancestor "$UP" "$BASE"; then
    if merge_probe_clean "$BASE" "$UP"; then
      echo "  probe: merge of upstream/main onto origin/main is CLEAN"
    else
      echo "  probe: merge of upstream/main onto origin/main has CONFLICTS"
      echo "         (open a dirty PR via: mise run upstream:sync -- --apply)"
    fi
  fi

  echo "  next: mise run upstream:dispatch [-- --watch]  # preferred apply (remote CI)"
  echo "        mise run upstream:sync                   # local dry-run plan only"
  echo "        mise run upstream:sync -- --apply        # break-glass local write"
  echo "  schedule: Upstream sync workflow every 6h UTC (00/06/12/18) — does not run on merge of automation alone"
}

run_upstream_sync() {
  # Flags already exported as FORCE_APPLY / FORCE_DRY_RUN by the task wrapper.
  local mode tip_pr

  upstream_bootstrap "upstream:sync"
  if should_apply; then
    mode=apply
  else
    mode=dry-run
  fi

  echo "upstream:sync mode=${mode} repo=${REPO}"
  load_upstream_tips
  echo "  origin/main:   ${BASE}"
  echo "  upstream/main: ${UP} (${SHORT})"
  echo "  branch:        ${BRANCH}"

  if is_ancestor "$UP" "$BASE"; then
    echo "  already integrated — would close all open sync PRs"
    if [[ "$mode" == "apply" ]]; then
      ensure_sync_labels
      close_all_sync_prs \
        "Closed by upstream:sync: \`upstream/main\` (\`${SHORT}\`) is already contained in \`main\`."
    fi
    echo "  done"
    return 0
  fi

  trap cleanup_build_worktree EXIT
  build_sync_merge "$BASE" "$UP"
  TIP_CLEAN=$BUILD_CLEAN
  TIP_HEAD=$BUILD_HEAD
  TIP_BRANCH=$BUILD_BRANCH
  echo "  tip merge: clean=${TIP_CLEAN} head=${TIP_HEAD}"

  if [[ "$mode" != "apply" ]]; then
    echo "  dry-run plan:"
    echo "    - push ${TIP_BRANCH} @ ${TIP_HEAD} (force-with-lease if exists)"
    if [[ "$TIP_CLEAN" == "true" ]]; then
      echo "    - ensure clean PR for this tip; close all other open sync PRs"
    else
      echo "    - ensure dirty PR for this tip (conflict markers committed)"
      echo "    - close other dirty sync PRs; keep/rebuild latest clean slot"
    fi
    echo "  currently open:"
    print_open_sync_prs "    "
    echo "  preferred apply: mise run upstream:dispatch [-- --watch]"
    echo "  break-glass:     mise run upstream:sync -- --apply"
    return 0
  fi

  apply_tip_sync
  cleanup_build_worktree
  trap - EXIT

  echo "  done — review open sync PRs; never auto-merge"
  echo "  diagnostic: mise run upstream:status"
}
