#!/usr/bin/env bash

source "$(dirname "$0")/_lib.sh"
require_neon_api_key

[[ $# -ge 2 ]] || err \
  "usage: $0 <branch> \"<sql>\" [direct|pooled] [project_id] [database]"

branch="$1"
sql="$2"
mode="${3:-pooled}"
project_id="${4:-}"
database="${5:-}"

neon_psql_c "$branch" "$sql" "$mode" "$project_id" "$database"
