#!/usr/bin/env bash
# Public-repo gate: fail if tracked files contain personal paths, emails, or secret-like strings.
# Locations must be resolved at runtime through the Platform layer, never hard-coded.
set -euo pipefail
cd "$(dirname "$0")/.."

patterns=(
  '/Users/[A-Za-z0-9._-]+'                 # macOS home paths
  '/home/[a-z_][a-z0-9_-]*/'               # Linux home paths
  '[A-Za-z]:\\+Users\\+'                   # Windows profile paths
  '[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}'  # email addresses
  'gh[pousr]_[A-Za-z0-9]{20,}'             # GitHub tokens
  'github_pat_[A-Za-z0-9_]{20,}'
  'sk-[A-Za-z0-9_-]{20,}'                  # API keys
  'AKIA[0-9A-Z]{16}'                       # AWS access keys
  'BEGIN [A-Z ]*PRIVATE KEY'
)
# Vendored Spec Kit files, lockfiles, and this script are excluded.
excludes=(':!.agents' ':!.specify/scripts' ':!*.lock' ':!package-lock.json' ':!scripts/check-hardcode.sh')
# Placeholder emails used in docs and tests.
allow='@(example\.(com|org)|users\.noreply\.github\.com)'

status=0
for p in "${patterns[@]}"; do
  if hits=$(git grep -nIE "$p" -- . "${excludes[@]}" | grep -vE "$allow"); then
    echo "hard-coded value matching /$p/:"
    echo "$hits"
    status=1
  fi
done
[ "$status" -eq 0 ] && echo "check-hardcode: OK"
exit "$status"
