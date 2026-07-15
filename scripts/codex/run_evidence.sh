#!/usr/bin/env bash

set -u

usage() {
  cat >&2 <<'EOF'
Usage: scripts/codex/run_evidence.sh \
  --task <task-slug> --category <category> --stem <short-name> \
  --inputs <description> [--metadata <task.json>] \
  [--artifact <kind=path>]... -- <command> [args...]
EOF
}

task=""
category=""
stem=""
inputs=""
metadata=""
artifacts=()

while (($#)); do
  case "$1" in
    --task | --category | --stem | --inputs | --metadata | --artifact)
      if (($# < 2)); then
        usage
        exit 64
      fi
      case "$1" in
        --task) task="$2" ;;
        --category) category="$2" ;;
        --stem) stem="$2" ;;
        --inputs) inputs="$2" ;;
        --metadata) metadata="$2" ;;
        --artifact) artifacts+=("$2") ;;
      esac
      shift 2
      ;;
    --)
      shift
      break
      ;;
    -h | --help)
      usage
      exit 0
      ;;
    *)
      printf 'unknown option: %s\n' "$1" >&2
      usage
      exit 64
      ;;
  esac
done

if [[ -z "$task" || -z "$category" || -z "$stem" || -z "$inputs" || $# -eq 0 ]]; then
  usage
  exit 64
fi
for value in "$task" "$category" "$stem"; do
  if [[ ! "$value" =~ ^[A-Za-z0-9._-]+$ ]]; then
    printf 'task, category, and stem accept only A-Z, a-z, 0-9, dot, underscore, and dash: %s\n' "$value" >&2
    exit 64
  fi
done

caller_cwd="$PWD"
script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
task_dir="$caller_cwd/target/codex-runs/$task"
manifest="$task_dir/manifest.json"
mkdir -p "$task_dir/commands" "$task_dir/reports"

while :; do
  run_id="$(date -u +%Y%m%dT%H%M%S-%N)-$$-${RANDOM}"
  command_dir="$task_dir/commands/$run_id"
  if mkdir "$command_dir" 2>/dev/null; then
    break
  fi
done

log="$command_dir/${category}-${stem}.log"
status_file="$command_dir/${category}-${stem}.status"
started_at="$(date -Is)"
started_ns="$(date +%s%N)"
command=("$@")
printf -v command_display '%q ' "${command[@]}"

{
  printf 'cwd=%s\n' "$caller_cwd"
  printf 'started_at=%s\n' "$started_at"
  printf 'task=%s\n' "$task"
  printf 'category=%s\n' "$category"
  printf 'stem=%s\n' "$stem"
  printf 'inputs=%q\n' "$inputs"
  printf 'command=%s\n' "$command_display"
  printf '%s\n' '--- stdout+stderr ---'
} >"$log"

set +e
"${command[@]}" >>"$log" 2>&1
rc=$?
set -e

ended_ns="$(date +%s%N)"
ended_at="$(date -Is)"
elapsed_seconds="$(python3 -c 'import sys; print((int(sys.argv[2]) - int(sys.argv[1])) / 1_000_000_000)' "$started_ns" "$ended_ns")"
if ((rc == 0)); then
  result="PASS"
else
  result="FAIL"
fi

{
  printf '\n%s\n' '--- evidence metadata ---'
  printf 'ended_at=%s\n' "$ended_at"
  printf 'elapsed_seconds=%s\n' "$elapsed_seconds"
  printf 'exit_code=%s\n' "$rc"
  printf 'result_summary=%s\n' "$result"
} >>"$log"
{
  printf 'run_id=%s\n' "$run_id"
  printf 'log=%s\n' "$log"
  printf 'exit_code=%s\n' "$rc"
  printf 'result_summary=%s\n' "$result"
} >"$status_file"

base_sha="$(git -C "$caller_cwd" rev-parse HEAD 2>/dev/null || true)"
manifest_args=(
  --manifest "$manifest"
  --task "$task"
  --base-sha "$base_sha"
  --cwd "$caller_cwd"
  --run-id "$run_id"
  --category "$category"
  --stem "$stem"
  --inputs "$inputs"
  --started-at "$started_at"
  --ended-at "$ended_at"
  --elapsed-seconds "$elapsed_seconds"
  --exit-code "$rc"
  --log "$log"
  --status-file "$status_file"
)
if [[ -n "$metadata" ]]; then
  manifest_args+=(--metadata "$metadata")
fi
for artifact in "${artifacts[@]}"; do
  manifest_args+=(--artifact "$artifact")
done

set +e
python3 "$script_dir/_manifest.py" "${manifest_args[@]}" --command "${command[@]}"
manifest_rc=$?
set -e
if ((manifest_rc != 0)); then
  printf 'manifest_update=FAIL manifest=%s\n' "$manifest" >&2
else
  printf 'result=%s exit_code=%s\n' "$result" "$rc"
  printf 'evidence_log=%s\nstatus_file=%s\nmanifest=%s\n' "$log" "$status_file" "$manifest"
fi

exit "$rc"
