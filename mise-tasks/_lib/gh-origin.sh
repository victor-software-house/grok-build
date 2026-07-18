# Resolve origin as owner/repo for `gh -R`, and pin gh's local default to the fork.

origin_name_with_owner() {
  local url nwo
  url=$(git remote get-url origin 2>/dev/null) || {
    echo "gh-origin: no origin remote" >&2
    return 1
  }
  url=${url%.git}
  url=${url%/}
  # sed: works under bash/zsh; avoid BASH_REMATCH when accidentally sourced.
  nwo=$(printf '%s\n' "$url" | sed -nE 's#.*github\.com[:/]([^/]+)/([^/]+)$#\1/\2#p')
  if [[ -z "$nwo" ]]; then
    echo "gh-origin: origin is not a github.com remote: $url" >&2
    return 1
  fi
  printf '%s\n' "$nwo"
}

# Idempotent: make bare `gh pr` / `gh run` hit the fork, not upstream.
ensure_gh_default_origin() {
  command -v gh >/dev/null 2>&1 || return 0
  git remote get-url origin >/dev/null 2>&1 || return 0
  # Prefer remote name when gh supports it; fall back to owner/repo.
  if gh repo set-default origin >/dev/null 2>&1; then
    return 0
  fi
  local nwo
  nwo=$(origin_name_with_owner) || return 0
  gh repo set-default "$nwo" >/dev/null 2>&1 || true
}
