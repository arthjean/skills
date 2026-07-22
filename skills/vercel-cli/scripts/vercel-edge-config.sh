#!/usr/bin/env bash
# vercel-edge-config.sh - deterministic Edge Config and item CRUD via REST API.
# Use when JSON patch files or stable raw responses are preferable to the native CLI.
#
# Usage:
#   ./vercel-edge-config.sh list
#   ./vercel-edge-config.sh get    <ec-id>
#   ./vercel-edge-config.sh create <slug>
#   ./vercel-edge-config.sh rm     <ec-id>
#   ./vercel-edge-config.sh items-list <ec-id>
#   ./vercel-edge-config.sh item-get  <ec-id> <key>
#   ./vercel-edge-config.sh upsert    <ec-id> <items.json>
#   ./vercel-edge-config.sh item-rm   <ec-id> <key>
#   ./vercel-edge-config.sh token-get <ec-id>
#
# items.json format (Vercel patch operations):
#   [
#     { "operation": "create", "key": "foo", "value": "bar" },
#     { "operation": "update", "key": "bar", "value": 42 },
#     { "operation": "upsert", "key": "baz", "value": {"nested": true} },
#     { "operation": "delete", "key": "qux" }
#   ]

source "$(dirname "$0")/_lib.sh"
require_vercel_token

[[ $# -ge 1 ]] || err "usage: $0 {list|get|create|rm|items-list|item-get|upsert|item-rm|token-get} ..."

action="$1"
shift

case "$action" in
  list)
    path=$(with_team_query "/v1/edge-config")
    vercel_api GET "$path" | jq .
    ;;

  get)
    [[ $# -ge 1 ]] || err "usage: $0 get <ec-id>"
    path=$(with_team_query "/v1/edge-config/$1")
    vercel_api GET "$path" | jq .
    ;;

  create)
    [[ $# -ge 1 ]] || err "usage: $0 create <slug>"
    body=$(jq -nc --arg slug "$1" '{slug: $slug}')
    path=$(with_team_query "/v1/edge-config")
    vercel_api POST "$path" "$body" | jq .
    ;;

  rm)
    [[ $# -ge 1 ]] || err "usage: $0 rm <ec-id>"
    path=$(with_team_query "/v1/edge-config/$1")
    vercel_api DELETE "$path" | jq .
    ;;

  items-list)
    [[ $# -ge 1 ]] || err "usage: $0 items-list <ec-id>"
    path=$(with_team_query "/v1/edge-config/$1/items")
    vercel_api GET "$path" | jq .
    ;;

  item-get)
    [[ $# -ge 2 ]] || err "usage: $0 item-get <ec-id> <key>"
    path=$(with_team_query "/v1/edge-config/$1/item/$2")
    vercel_api GET "$path" | jq .
    ;;

  upsert)
    [[ $# -ge 2 ]] || err "usage: $0 upsert <ec-id> <items.json>"
    ec_id="$1"; file="$2"
    [[ -f "$file" ]] || err "items file not found: $file"
    items_json=$(jq -c . "$file")
    body=$(jq -nc --argjson items "$items_json" '{items: $items}')
    path=$(with_team_query "/v1/edge-config/$ec_id/items")
    vercel_api PATCH "$path" "$body" | jq .
    ;;

  item-rm)
    [[ $# -ge 2 ]] || err "usage: $0 item-rm <ec-id> <key>"
    body=$(jq -nc --arg key "$2" '{items: [{operation: "delete", key: $key}]}')
    path=$(with_team_query "/v1/edge-config/$1/items")
    vercel_api PATCH "$path" "$body" | jq .
    ;;

  token-get)
    [[ $# -ge 1 ]] || err "usage: $0 token-get <ec-id>"
    path=$(with_team_query "/v1/edge-config/$1/token")
    vercel_api GET "$path" | jq .
    ;;

  *)
    err "unknown action: $action"
    ;;
esac
