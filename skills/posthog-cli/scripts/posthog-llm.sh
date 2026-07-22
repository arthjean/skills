#!/usr/bin/env bash
# posthog-llm.sh - LLM analytics: prompts, evaluations, sentiment, summarization, trace reviews,
# review queues, skills (PostHog LLM analytics resources, not Codex skills), clustering jobs.
# Replaces MCP tools: llma-prompt-*, llma-evaluation-*, llma-evaluation-config-*,
#                     llma-evaluation-report-*, llma-evaluation-judge-models, llma-evaluation-test-hog,
#                     llma-sentiment-create, llma-summarization-create, llma-trace-review-*,
#                     llma-review-queue-*, llma-review-queue-item-*, llma-skill-*, llma-skill-file-*,
#                     llma-clustering-job-list, llma-clustering-job-get,
#                     get-llm-total-costs-for-project (HogQL recipe lives in references/hogql-cookbook.md).
#
# Usage examples (most subcommands accept [project_id] as the LAST positional arg):
#   ./posthog-llm.sh prompts ls
#   ./posthog-llm.sh prompts get        <id>
#   ./posthog-llm.sh prompts create     <body-json>
#   ./posthog-llm.sh prompts update     <id> <patch-json>
#   ./posthog-llm.sh prompts duplicate  <id>
#   ./posthog-llm.sh evaluations ls
#   ./posthog-llm.sh evaluations get    <id>
#   ./posthog-llm.sh evaluations create <body-json>
#   ./posthog-llm.sh evaluations update <id> <patch-json>
#   ./posthog-llm.sh evaluations rm     <id>
#   ./posthog-llm.sh evaluations run    <id>
#   ./posthog-llm.sh evaluations judge-models
#   ./posthog-llm.sh evaluations test-hog <body-json>
#   ./posthog-llm.sh eval-config get
#   ./posthog-llm.sh eval-config set-active <key>
#   ./posthog-llm.sh reports ls
#   ./posthog-llm.sh reports get        <id>
#   ./posthog-llm.sh reports create     <body-json>
#   ./posthog-llm.sh reports update     <id> <patch-json>
#   ./posthog-llm.sh reports rm         <id>
#   ./posthog-llm.sh reports generate   <id>
#   ./posthog-llm.sh reports runs       <id>
#   ./posthog-llm.sh sentiment   <body-json>
#   ./posthog-llm.sh summarize   <body-json>
#   ./posthog-llm.sh reviews ls
#   ./posthog-llm.sh reviews get <id>
#   ./posthog-llm.sh reviews create <body-json>
#   ./posthog-llm.sh reviews update <id> <patch-json>
#   ./posthog-llm.sh reviews rm <id>
#   ./posthog-llm.sh queues ls
#   ./posthog-llm.sh queues get <id>
#   ./posthog-llm.sh queues create <body-json>
#   ./posthog-llm.sh queues update <id> <patch-json>
#   ./posthog-llm.sh queues rm <id>
#   ./posthog-llm.sh queue-items ls    <queue_id>
#   ./posthog-llm.sh queue-items add   <queue_id> <body-json>
#   ./posthog-llm.sh queue-items get   <queue_id> <item_id>
#   ./posthog-llm.sh queue-items update <queue_id> <item_id> <patch-json>
#   ./posthog-llm.sh queue-items rm    <queue_id> <item_id>
#   ./posthog-llm.sh skills ls
#   ./posthog-llm.sh skills get        <id>
#   ./posthog-llm.sh skills create     <body-json>
#   ./posthog-llm.sh skills duplicate  <id>
#   ./posthog-llm.sh skill-files ls    <skill_id>
#   ./posthog-llm.sh skill-files add   <skill_id> <body-json>
#   ./posthog-llm.sh skill-files get   <skill_id> <file_id>
#   ./posthog-llm.sh skill-files rename <skill_id> <file_id> <new_name>
#   ./posthog-llm.sh skill-files rm    <skill_id> <file_id>
#   ./posthog-llm.sh clusters ls
#   ./posthog-llm.sh clusters get      <job_id>

source "$(dirname "$0")/_lib.sh"
require_posthog_key

[[ $# -ge 1 ]] || err "usage: $0 {prompts|evaluations|eval-config|reports|sentiment|summarize|reviews|queues|queue-items|skills|skill-files|clusters} <subcommand> [args...]"

resource="$1"; shift
[[ $# -ge 1 ]] || err "missing subcommand for $resource"

# All endpoints are project-scoped under /api/environments/{id}/llm_observability/... in the new
# layout, but PostHog also accepts /api/projects/{id}/. We use the projects/ prefix for consistency.
base_for() {
  local pid="$1" sub="$2"
  printf '/api/projects/%s/llm_observability/%s' "$pid" "$sub"
}

case "$resource" in
  prompts)
    sub="$1"; shift
    case "$sub" in
      ls)        pid=$(resolve_project_id "${1:-}"); posthog_api GET  "$(base_for "$pid" "prompts/")" | pretty ;;
      get)       [[ $# -ge 1 ]] || err "prompts get <id>"
                 pid=$(resolve_project_id "${2:-}"); posthog_api GET  "$(base_for "$pid" "prompts/$1/")" | pretty ;;
      create)    [[ $# -ge 1 ]] || err "prompts create <body-json>"
                 pid=$(resolve_project_id "${2:-}"); posthog_api POST "$(base_for "$pid" "prompts/")" "$1" | pretty ;;
      update)    [[ $# -ge 2 ]] || err "prompts update <id> <patch-json>"
                 pid=$(resolve_project_id "${3:-}"); posthog_api PATCH "$(base_for "$pid" "prompts/$1/")" "$2" | pretty ;;
      duplicate) [[ $# -ge 1 ]] || err "prompts duplicate <id>"
                 pid=$(resolve_project_id "${2:-}"); posthog_api POST "$(base_for "$pid" "prompts/$1/duplicate/")" | pretty ;;
      *) err "unknown prompts subcommand: $sub" ;;
    esac ;;

  evaluations)
    sub="$1"; shift
    case "$sub" in
      ls)         pid=$(resolve_project_id "${1:-}"); posthog_api GET  "$(base_for "$pid" "evaluations/")" | pretty ;;
      get)        [[ $# -ge 1 ]] || err "evaluations get <id>"
                  pid=$(resolve_project_id "${2:-}"); posthog_api GET  "$(base_for "$pid" "evaluations/$1/")" | pretty ;;
      create)     [[ $# -ge 1 ]] || err "evaluations create <body-json>"
                  pid=$(resolve_project_id "${2:-}"); posthog_api POST "$(base_for "$pid" "evaluations/")" "$1" | pretty ;;
      update)     [[ $# -ge 2 ]] || err "evaluations update <id> <patch-json>"
                  pid=$(resolve_project_id "${3:-}"); posthog_api PATCH "$(base_for "$pid" "evaluations/$1/")" "$2" | pretty ;;
      rm)         [[ $# -ge 1 ]] || err "evaluations rm <id>"
                  pid=$(resolve_project_id "${2:-}"); posthog_api DELETE "$(base_for "$pid" "evaluations/$1/")" | pretty ;;
      run)        [[ $# -ge 1 ]] || err "evaluations run <id>"
                  pid=$(resolve_project_id "${2:-}"); posthog_api POST "$(base_for "$pid" "evaluations/$1/run/")" | pretty ;;
      judge-models) pid=$(resolve_project_id "${1:-}"); posthog_api GET "$(base_for "$pid" "evaluations/judge_models/")" | pretty ;;
      test-hog)   [[ $# -ge 1 ]] || err "evaluations test-hog <body-json>"
                  pid=$(resolve_project_id "${2:-}"); posthog_api POST "$(base_for "$pid" "evaluations/test_hog/")" "$1" | pretty ;;
      *) err "unknown evaluations subcommand: $sub" ;;
    esac ;;

  eval-config)
    sub="$1"; shift
    case "$sub" in
      get)        pid=$(resolve_project_id "${1:-}"); posthog_api GET  "$(base_for "$pid" "evaluation_config/")" | pretty ;;
      set-active) [[ $# -ge 1 ]] || err "eval-config set-active <key>"
                  pid=$(resolve_project_id "${2:-}")
                  body=$(jq -nc --arg k "$1" '{key:$k}')
                  posthog_api POST "$(base_for "$pid" "evaluation_config/set_active_key/")" "$body" | pretty ;;
      *) err "unknown eval-config subcommand: $sub" ;;
    esac ;;

  reports)
    sub="$1"; shift
    case "$sub" in
      ls)       pid=$(resolve_project_id "${1:-}"); posthog_api GET  "$(base_for "$pid" "evaluation_reports/")" | pretty ;;
      get)      [[ $# -ge 1 ]] || err "reports get <id>"
                pid=$(resolve_project_id "${2:-}"); posthog_api GET  "$(base_for "$pid" "evaluation_reports/$1/")" | pretty ;;
      create)   [[ $# -ge 1 ]] || err "reports create <body-json>"
                pid=$(resolve_project_id "${2:-}"); posthog_api POST "$(base_for "$pid" "evaluation_reports/")" "$1" | pretty ;;
      update)   [[ $# -ge 2 ]] || err "reports update <id> <patch-json>"
                pid=$(resolve_project_id "${3:-}"); posthog_api PATCH "$(base_for "$pid" "evaluation_reports/$1/")" "$2" | pretty ;;
      rm)       [[ $# -ge 1 ]] || err "reports rm <id>"
                pid=$(resolve_project_id "${2:-}"); posthog_api DELETE "$(base_for "$pid" "evaluation_reports/$1/")" | pretty ;;
      generate) [[ $# -ge 1 ]] || err "reports generate <id>"
                pid=$(resolve_project_id "${2:-}"); posthog_api POST "$(base_for "$pid" "evaluation_reports/$1/generate/")" | pretty ;;
      runs)     [[ $# -ge 1 ]] || err "reports runs <id>"
                pid=$(resolve_project_id "${2:-}"); posthog_api GET "$(base_for "$pid" "evaluation_reports/$1/runs/")" | pretty ;;
      *) err "unknown reports subcommand: $sub" ;;
    esac ;;

  sentiment)
    [[ $# -ge 1 ]] || err "sentiment <body-json> [project_id]"
    pid=$(resolve_project_id "${2:-}"); posthog_api POST "$(base_for "$pid" "sentiment/")" "$1" | pretty ;;
  summarize)
    [[ $# -ge 1 ]] || err "summarize <body-json> [project_id]"
    pid=$(resolve_project_id "${2:-}"); posthog_api POST "$(base_for "$pid" "summarization/")" "$1" | pretty ;;

  reviews)
    sub="$1"; shift
    case "$sub" in
      ls)     pid=$(resolve_project_id "${1:-}"); posthog_api GET  "$(base_for "$pid" "trace_reviews/")" | pretty ;;
      get)    [[ $# -ge 1 ]] || err "reviews get <id>"
              pid=$(resolve_project_id "${2:-}"); posthog_api GET  "$(base_for "$pid" "trace_reviews/$1/")" | pretty ;;
      create) [[ $# -ge 1 ]] || err "reviews create <body-json>"
              pid=$(resolve_project_id "${2:-}"); posthog_api POST "$(base_for "$pid" "trace_reviews/")" "$1" | pretty ;;
      update) [[ $# -ge 2 ]] || err "reviews update <id> <patch-json>"
              pid=$(resolve_project_id "${3:-}"); posthog_api PATCH "$(base_for "$pid" "trace_reviews/$1/")" "$2" | pretty ;;
      rm)     [[ $# -ge 1 ]] || err "reviews rm <id>"
              pid=$(resolve_project_id "${2:-}"); posthog_api DELETE "$(base_for "$pid" "trace_reviews/$1/")" | pretty ;;
      *) err "unknown reviews subcommand: $sub" ;;
    esac ;;

  queues)
    sub="$1"; shift
    case "$sub" in
      ls)     pid=$(resolve_project_id "${1:-}"); posthog_api GET  "$(base_for "$pid" "review_queues/")" | pretty ;;
      get)    [[ $# -ge 1 ]] || err "queues get <id>"
              pid=$(resolve_project_id "${2:-}"); posthog_api GET  "$(base_for "$pid" "review_queues/$1/")" | pretty ;;
      create) [[ $# -ge 1 ]] || err "queues create <body-json>"
              pid=$(resolve_project_id "${2:-}"); posthog_api POST "$(base_for "$pid" "review_queues/")" "$1" | pretty ;;
      update) [[ $# -ge 2 ]] || err "queues update <id> <patch-json>"
              pid=$(resolve_project_id "${3:-}"); posthog_api PATCH "$(base_for "$pid" "review_queues/$1/")" "$2" | pretty ;;
      rm)     [[ $# -ge 1 ]] || err "queues rm <id>"
              pid=$(resolve_project_id "${2:-}"); posthog_api DELETE "$(base_for "$pid" "review_queues/$1/")" | pretty ;;
      *) err "unknown queues subcommand: $sub" ;;
    esac ;;

  queue-items)
    sub="$1"; shift
    [[ $# -ge 1 ]] || err "queue-items $sub <queue_id> [...]"
    qid="$1"; shift
    case "$sub" in
      ls)     pid=$(resolve_project_id "${1:-}"); posthog_api GET  "$(base_for "$pid" "review_queues/$qid/items/")" | pretty ;;
      add)    [[ $# -ge 1 ]] || err "queue-items add <queue_id> <body-json>"
              pid=$(resolve_project_id "${2:-}"); posthog_api POST "$(base_for "$pid" "review_queues/$qid/items/")" "$1" | pretty ;;
      get)    [[ $# -ge 1 ]] || err "queue-items get <queue_id> <item_id>"
              pid=$(resolve_project_id "${2:-}"); posthog_api GET  "$(base_for "$pid" "review_queues/$qid/items/$1/")" | pretty ;;
      update) [[ $# -ge 2 ]] || err "queue-items update <queue_id> <item_id> <patch-json>"
              pid=$(resolve_project_id "${3:-}"); posthog_api PATCH "$(base_for "$pid" "review_queues/$qid/items/$1/")" "$2" | pretty ;;
      rm)     [[ $# -ge 1 ]] || err "queue-items rm <queue_id> <item_id>"
              pid=$(resolve_project_id "${2:-}"); posthog_api DELETE "$(base_for "$pid" "review_queues/$qid/items/$1/")" | pretty ;;
      *) err "unknown queue-items subcommand: $sub" ;;
    esac ;;

  skills)
    sub="$1"; shift
    case "$sub" in
      ls)        pid=$(resolve_project_id "${1:-}"); posthog_api GET  "$(base_for "$pid" "skills/")" | pretty ;;
      get)       [[ $# -ge 1 ]] || err "skills get <id>"
                 pid=$(resolve_project_id "${2:-}"); posthog_api GET  "$(base_for "$pid" "skills/$1/")" | pretty ;;
      create)    [[ $# -ge 1 ]] || err "skills create <body-json>"
                 pid=$(resolve_project_id "${2:-}"); posthog_api POST "$(base_for "$pid" "skills/")" "$1" | pretty ;;
      duplicate) [[ $# -ge 1 ]] || err "skills duplicate <id>"
                 pid=$(resolve_project_id "${2:-}"); posthog_api POST "$(base_for "$pid" "skills/$1/duplicate/")" | pretty ;;
      *) err "unknown skills subcommand: $sub" ;;
    esac ;;

  skill-files)
    sub="$1"; shift
    [[ $# -ge 1 ]] || err "skill-files $sub <skill_id> [...]"
    sid="$1"; shift
    case "$sub" in
      ls)     pid=$(resolve_project_id "${1:-}"); posthog_api GET  "$(base_for "$pid" "skills/$sid/files/")" | pretty ;;
      add)    [[ $# -ge 1 ]] || err "skill-files add <skill_id> <body-json>"
              pid=$(resolve_project_id "${2:-}"); posthog_api POST "$(base_for "$pid" "skills/$sid/files/")" "$1" | pretty ;;
      get)    [[ $# -ge 1 ]] || err "skill-files get <skill_id> <file_id>"
              pid=$(resolve_project_id "${2:-}"); posthog_api GET  "$(base_for "$pid" "skills/$sid/files/$1/")" | pretty ;;
      rename) [[ $# -ge 2 ]] || err "skill-files rename <skill_id> <file_id> <new_name>"
              pid=$(resolve_project_id "${3:-}")
              body=$(jq -nc --arg n "$2" '{name:$n}')
              posthog_api POST "$(base_for "$pid" "skills/$sid/files/$1/rename/")" "$body" | pretty ;;
      rm)     [[ $# -ge 1 ]] || err "skill-files rm <skill_id> <file_id>"
              pid=$(resolve_project_id "${2:-}"); posthog_api DELETE "$(base_for "$pid" "skills/$sid/files/$1/")" | pretty ;;
      *) err "unknown skill-files subcommand: $sub" ;;
    esac ;;

  clusters)
    sub="$1"; shift
    case "$sub" in
      ls)  pid=$(resolve_project_id "${1:-}"); posthog_api GET  "$(base_for "$pid" "clustering_jobs/")" | pretty ;;
      get) [[ $# -ge 1 ]] || err "clusters get <job_id>"
           pid=$(resolve_project_id "${2:-}"); posthog_api GET  "$(base_for "$pid" "clustering_jobs/$1/")" | pretty ;;
      *) err "unknown clusters subcommand: $sub" ;;
    esac ;;

  *) err "unknown resource: $resource" ;;
esac
