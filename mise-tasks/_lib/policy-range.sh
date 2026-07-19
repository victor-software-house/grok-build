# Policy range helpers: which commits in a git range to identity/commitlint.
# shellcheck shell=bash
#
# Normal PRs/pushes: every commit in the range.
# Upstream sync only (branch sync/upstream-<hex> from our automation):
#   skip commits that are ancestors of that upstream tip (imported history).
#   Still check our merge/resolution commits on the sync branch.

UPSTREAM_SYNC_BRANCH_RE='^sync/upstream-([0-9a-fA-F]+)$'

# Resolve current head branch name (PR CI → GITHUB_HEAD_REF, else local).
policy_head_branch() {
  if [[ -n "${GITHUB_HEAD_REF:-}" ]]; then
    printf '%s' "$GITHUB_HEAD_REF"
    return 0
  fi
  local b
  b=$(git branch --show-current 2>/dev/null || true)
  if [[ -n "$b" ]]; then
    printf '%s' "$b"
    return 0
  fi
  return 1
}

# If this is an automation sync branch, print full upstream tip SHA; else return 1.
policy_sync_upstream_tip() {
  # Explicit override (tests / break-glass).
  if [[ -n "${SYNC_UPSTREAM_TIP:-}" ]]; then
    git rev-parse --verify "${SYNC_UPSTREAM_TIP}^{commit}" 2>/dev/null
    return
  fi

  local branch suffix tip
  branch=$(policy_head_branch) || return 1
  if [[ ! "$branch" =~ $UPSTREAM_SYNC_BRANCH_RE ]]; then
    return 1
  fi
  suffix="${BASH_REMATCH[1]}"
  tip=$(git rev-parse --verify "${suffix}^{commit}" 2>/dev/null) || {
    # Short name: first match in the range or repo.
    tip=$(git rev-list --all | grep -E "^${suffix}" | head -1 || true)
    [[ -n "$tip" ]] || return 1
  }
  printf '%s' "$tip"
}

# Print SHAs in RANGE that policy should validate (one per line).
# On sync/upstream-<hex> (or SYNC_UPSTREAM_TIP): omit commits ancestral to that tip.
policy_commits_to_check() {
  local range=$1
  local tip sha

  if tip=$(policy_sync_upstream_tip); then
    echo "policy-range: sync automation — skip commits ancestral to upstream tip ${tip:0:12}" >&2
    while IFS= read -r sha; do
      [[ -z "$sha" ]] && continue
      if git merge-base --is-ancestor "$sha" "$tip" 2>/dev/null; then
        continue
      fi
      printf '%s\n' "$sha"
    done < <(git rev-list --reverse "$range")
  else
    git rev-list --reverse "$range"
  fi
}
