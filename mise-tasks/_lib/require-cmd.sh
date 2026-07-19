# Fail if a required command is missing.
# Usage: require_cmd jq   or   require_cmd jq "pr:merge"

require_cmd() {
  local cmd=$1
  local who=${2:-$(basename "$0")}
  command -v "$cmd" >/dev/null 2>&1 || {
    printf '%s: %s is required\n' "$who" "$cmd" >&2
    return 2
  }
}
