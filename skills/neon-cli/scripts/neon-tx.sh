#!/usr/bin/env bash

source "$(dirname "$0")/_lib.sh"
require_neon_api_key

[[ $# -ge 1 ]] || err \
  "usage: $0 <branch> [-f file.sql] [direct|pooled] [project_id] [database]"

branch="$1"
shift
file=""

if [[ "${1:-}" == "-f" ]]; then
  shift
  file="${1:?missing filename after -f}"
  shift
fi

mode="${1:-direct}"
project_id="${2:-}"
database="${3:-}"

neon_psql_tx "$branch" "$file" "$mode" "$project_id" "$database"
