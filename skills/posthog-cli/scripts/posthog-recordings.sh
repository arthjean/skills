#!/usr/bin/env bash
# posthog-recordings.sh - session recordings + playlists + summarize.
# Replaces MCP tools: session-recording-get, session-recording-delete, session-recording-summarize,
#                     session-recording-playlist-get, session-recording-playlist-create,
#                     session-recording-playlist-update, session-recording-playlists-list.
#
# Usage:
#   ./posthog-recordings.sh ls           [limit]                            [project_id]
#   ./posthog-recordings.sh get          <recording_id>                     [project_id]
#   ./posthog-recordings.sh snapshots    <recording_id>                     [project_id]
#   ./posthog-recordings.sh summarize    <recording_id>                     [project_id]
#   ./posthog-recordings.sh rm           <recording_id>                     [project_id]
#   ./posthog-recordings.sh playlists                                       [project_id]
#   ./posthog-recordings.sh playlist-get <playlist_id>                      [project_id]
#   ./posthog-recordings.sh playlist-create <name>                          [project_id]
#   ./posthog-recordings.sh playlist-update <playlist_id> <patch-json>      [project_id]
#   ./posthog-recordings.sh url          <recording_id>                     [project_id]

source "$(dirname "$0")/_lib.sh"
require_posthog_key

[[ $# -ge 1 ]] || err "usage: $0 {ls|get|snapshots|summarize|rm|playlists|playlist-get|playlist-create|playlist-update|url} [args...]"

action="$1"; shift

case "$action" in
  ls)
    limit="${1:-50}"; pid=$(resolve_project_id "${2:-}")
    posthog_api GET "/api/projects/$pid/session_recordings/?limit=${limit}" | pretty ;;
  get)
    [[ $# -ge 1 ]] || err "usage: $0 get <recording_id> [project_id]"
    pid=$(resolve_project_id "${2:-}")
    posthog_api GET "/api/projects/$pid/session_recordings/$1/" | pretty ;;
  snapshots)
    [[ $# -ge 1 ]] || err "usage: $0 snapshots <recording_id> [project_id]"
    pid=$(resolve_project_id "${2:-}")
    posthog_api GET "/api/projects/$pid/session_recordings/$1/snapshots/" | pretty ;;
  summarize)
    [[ $# -ge 1 ]] || err "usage: $0 summarize <recording_id> [project_id]"
    pid=$(resolve_project_id "${2:-}")
    posthog_api POST "/api/projects/$pid/session_recordings/$1/summarize/" | pretty ;;
  rm)
    [[ $# -ge 1 ]] || err "usage: $0 rm <recording_id> [project_id]"
    pid=$(resolve_project_id "${2:-}")
    posthog_api DELETE "/api/projects/$pid/session_recordings/$1/" | pretty ;;
  playlists)
    pid=$(resolve_project_id "${1:-}")
    posthog_api GET "/api/projects/$pid/session_recording_playlists/?limit=200" | pretty ;;
  playlist-get)
    [[ $# -ge 1 ]] || err "usage: $0 playlist-get <playlist_id> [project_id]"
    pid=$(resolve_project_id "${2:-}")
    posthog_api GET "/api/projects/$pid/session_recording_playlists/$1/" | pretty ;;
  playlist-create)
    [[ $# -ge 1 ]] || err "usage: $0 playlist-create <name> [project_id]"
    pid=$(resolve_project_id "${2:-}")
    body=$(jq -nc --arg n "$1" '{name:$n}')
    posthog_api POST "/api/projects/$pid/session_recording_playlists/" "$body" | pretty ;;
  playlist-update)
    [[ $# -ge 2 ]] || err "usage: $0 playlist-update <playlist_id> <patch-json> [project_id]"
    pid=$(resolve_project_id "${3:-}")
    posthog_api PATCH "/api/projects/$pid/session_recording_playlists/$1/" "$2" | pretty ;;
  url)
    [[ $# -ge 1 ]] || err "usage: $0 url <recording_id> [project_id]"
    pid=$(resolve_project_id "${2:-}")
    printf '%s/project/%s/replay/%s\n' "$POSTHOG_HOST" "$pid" "$1" ;;
  *) err "unknown action: $action" ;;
esac
