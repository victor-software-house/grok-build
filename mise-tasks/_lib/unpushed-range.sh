# Shared: git range of commits not yet on the remote tracking branch.
# Prefer @{upstream}..HEAD; for a new branch with no upstream, origin/main..HEAD.
# No artificial HEAD~N windows — only what is actually unpushed.
#
# Sets: UNPUSHED_RANGE
# Exits 2 if neither upstream nor origin/main exists (pass an explicit range).

unpushed_range() {
  if git rev-parse --abbrev-ref --symbolic-full-name '@{upstream}' >/dev/null 2>&1; then
    UNPUSHED_RANGE='@{upstream}..HEAD'
    return 0
  fi
  if git rev-parse --verify --quiet 'origin/main' >/dev/null; then
    UNPUSHED_RANGE='origin/main..HEAD'
    return 0
  fi
  echo "no @{upstream} and no origin/main — pass an explicit range" >&2
  return 2
}
