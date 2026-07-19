# Filter a git range to commits authored by "us" (config/our-commit-emails).
# shellcheck shell=bash
#
# load_our_email_patterns  → sets OUR_EMAIL_PATTERNS array
# is_our_email EMAIL       → 0 if matches
# for_our_commits RANGE    → prints SHAs (one per line) we should policy-check

load_our_email_patterns() {
  local root file line
  root=$(git rev-parse --show-toplevel)
  file="${root}/config/our-commit-emails"
  OUR_EMAIL_PATTERNS=()
  [[ -f "$file" ]] || {
    echo "our-commits: missing $file" >&2
    return 2
  }
  while IFS= read -r line || [[ -n "$line" ]]; do
    line="${line%%#*}"
    line="${line#"${line%%[![:space:]]*}"}"
    line="${line%"${line##*[![:space:]]}"}"
    [[ -z "$line" ]] && continue
    OUR_EMAIL_PATTERNS+=("$line")
  done <"$file"
  [[ ${#OUR_EMAIL_PATTERNS[@]} -gt 0 ]] || {
    echo "our-commits: empty $file" >&2
    return 2
  }
}

is_our_email() {
  local email=$1 p
  for p in "${OUR_EMAIL_PATTERNS[@]}"; do
    # shellcheck disable=SC2053
    [[ "$email" =~ $p ]] && return 0
  done
  return 1
}

# Print SHAs in RANGE whose *author* email is ours (newest-last order from git log).
for_our_commits() {
  local range=$1
  local sha author
  while IFS="$(printf '\t')" read -r sha author; do
    [[ -z "$sha" ]] && continue
    if is_our_email "$author"; then
      printf '%s\n' "$sha"
    fi
  done < <(git log --format='%H%x09%ae' "$range")
}
