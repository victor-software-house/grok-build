# Shared Conventional Commits subject rules for this fork.

types='feat|fix|docs|style|refactor|perf|test|build|ci|chore|revert'
subject_re="^(${types})(\\([a-zA-Z0-9._/-]+\\))?(!)?: .+"

validate_subject() {
  local subject="$1"
  local where="${2:-}"

  if [[ -z "${subject// }" ]]; then
    echo "commitlint: empty subject${where:+ ($where)}" >&2
    return 1
  fi

  if [[ "$subject" =~ ^Merge\  ]] || [[ "$subject" =~ ^Revert\  ]]; then
    return 0
  fi

  if [[ ! "$subject" =~ $subject_re ]]; then
    cat >&2 <<EOF
commitlint: not Conventional Commits${where:+ ($where)}:

  $subject

Expected: type(scope)?: subject
Types: feat fix docs style refactor perf test build ci chore revert
EOF
    return 1
  fi

  if [[ ${#subject} -gt 100 ]]; then
    echo "commitlint: subject >100 chars (${#subject})${where:+ ($where)}" >&2
    return 1
  fi

  if [[ "$subject" =~ \.$ ]]; then
    echo "commitlint: subject must not end with a period${where:+ ($where)}" >&2
    return 1
  fi

  return 0
}
