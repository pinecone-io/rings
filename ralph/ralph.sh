#!/usr/bin/env bash
# spec-extract.sh — Ralph-style iterative build/review loop for rings
# Phase 1: builder — picks one task from the plan and implements it.
# Phase 2: reviewer — reviews code quality and adds tasks if needed; loops back.
# Runs claude directly on the host.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
BUILD_PROMPT_MD="$SCRIPT_DIR/BUILD_PROMPT.md"
REVIEW_PROMPT_MD="$SCRIPT_DIR/REVIEW_PROMPT.md"
PLAN_FILE="$PROJECT_ROOT/docs/superpowers/plans/2026-03-15-rings-mvp.md"
USAGE_LOG="$SCRIPT_DIR/usage.jsonl"
MAX_ITERATIONS="${MAX_ITERATIONS:-50}"
MAX_REVIEW_PASSES="${MAX_REVIEW_PASSES:-5}"
BUILD_PER_REVIEW="${BUILD_PER_REVIEW:-2}"
MODEL="${MODEL:-claude-haiku-4-5-20251001}"

log() { echo "[rings-build] $*"; }

# ── Safety warning ───────────────────────────────────────────────────────────

warn_dangerous() {
  echo ""
  echo "  ⚠️  WARNING: This script runs claude with --dangerously-skip-permissions"
  echo "  Claude will be able to read, write, and execute without confirmation."
  echo "  Only proceed on a machine where this is safe (e.g. an isolated dev VM)."
  echo ""
  printf "  Continue? [y/N] "
  read -r answer
  case "$answer" in
    [yY]) echo "" ;;
    *) log "Aborted."; exit 1 ;;
  esac
}

# ── Plan helpers ─────────────────────────────────────────────────────────────

has_unchecked_tasks() {
  grep -q '^\- \[ \]' "$PLAN_FILE"
}

# ── Token usage logging ──────────────────────────────────────────────────────

# Run claude with --output-format json, log usage to usage.jsonl, return text output.
# Usage: run_claude <phase> <label> <system_prompt_file> <prompt>
run_claude() {
  local phase="$1"
  local label="$2"
  local system_prompt_file="$3"
  local prompt="$4"

  local raw
  raw=$(claude \
    --dangerously-skip-permissions \
    --output-format json \
    --model "$MODEL" \
    --system-prompt-file "$system_prompt_file" \
    --print \
    "$prompt")

  # Append a usage record to usage.jsonl
  echo "$raw" | python3 -c "
import json, sys, datetime
d = json.load(sys.stdin)
record = {
  'timestamp': datetime.datetime.utcnow().isoformat() + 'Z',
  'phase': '$phase',
  'label': '$label',
  'duration_ms': d.get('duration_ms'),
  'total_cost_usd': d.get('total_cost_usd'),
  'usage': d.get('usage', {}),
  'model_usage': d.get('modelUsage', {}),
}
print(json.dumps(record))
" >> "$USAGE_LOG"

  # Return just the text result for the caller
  echo "$raw" | python3 -c "import json,sys; print(json.load(sys.stdin).get('result', ''))"
}

# ── Usage summary ───────────────────────────────────────────────────────────

print_usage_summary() {
  python3 - "$USAGE_LOG" <<'EOF'
import json, sys
records = [json.loads(l) for l in open(sys.argv[1])]
total_cost    = sum(r.get("total_cost_usd", 0) for r in records)
input_tokens  = sum(r.get("usage", {}).get("input_tokens", 0) for r in records)
output_tokens = sum(r.get("usage", {}).get("output_tokens", 0) for r in records)
cache_read    = sum(r.get("usage", {}).get("cache_read_input_tokens", 0) for r in records)
cache_create  = sum(r.get("usage", {}).get("cache_creation_input_tokens", 0) for r in records)
print("")
print("  ── Token Usage Summary ──────────────────────────")
print(f"  Invocations:          {len(records)}")
print(f"  Input tokens:         {input_tokens:,}")
print(f"  Output tokens:        {output_tokens:,}")
print(f"  Cache read tokens:    {cache_read:,}")
print(f"  Cache created tokens: {cache_create:,}")
print(f"  Total cost (USD):     ${total_cost:.4f}")
print("  ─────────────────────────────────────────────────")
EOF
}

# ── Signal handling ─────────────────────────────────────────────────────────

trap 'echo ""; log "Interrupted."; print_usage_summary; exit 130' INT TERM

# ── Main ─────────────────────────────────────────────────────────────────────

main() {
  warn_dangerous
  log "Starting rings build loop"
  log "Project: $PROJECT_ROOT"
  log "Plan:    $PLAN_FILE"
  log "Token usage log: $USAGE_LOG"

  iteration=0
  review_pass=0

  while true; do

    # ── Phase 1: builder ────────────────────────────────────────────────────
    builds_this_cycle=0
    while has_unchecked_tasks && [[ $builds_this_cycle -lt $BUILD_PER_REVIEW ]]; do
      iteration=$((iteration + 1))
      builds_this_cycle=$((builds_this_cycle + 1))
      if [[ $iteration -gt $MAX_ITERATIONS ]]; then
        log "ERROR: Reached max iterations ($MAX_ITERATIONS). Stopping."
        print_usage_summary
        exit 1
      fi

      log "Build iteration $iteration: running builder..."

      build_output=$(run_claude \
        "build" \
        "iter-$iteration" \
        "$BUILD_PROMPT_MD" \
        "Read the plan at docs/superpowers/plans/2026-03-15-rings-mvp.md, pick the most important unchecked task, and implement it following BUILD_PROMPT.md.")

      echo "$build_output"

      if echo "$build_output" | grep -q "^RINGS_DONE"; then
        log "Builder signals completion — no tasks remain."
        print_usage_summary
        exit 0
      fi

      if echo "$build_output" | grep -q "^ITERATION COMPLETE"; then
        log "Build iteration $iteration complete."
      else
        log "WARNING: Build iteration $iteration did not emit ITERATION COMPLETE — continuing anyway."
      fi
    done

    # ── Phase 2: reviewer ───────────────────────────────────────────────────
    review_pass=$((review_pass + 1))
    if [[ $review_pass -gt $MAX_REVIEW_PASSES ]]; then
      log "ERROR: Reached max review passes ($MAX_REVIEW_PASSES). Stopping."
      print_usage_summary
      exit 1
    fi

    log "Review pass $review_pass: no unchecked tasks remain, starting review..."

    review_output=$(run_claude \
      "review" \
      "pass-$review_pass" \
      "$REVIEW_PROMPT_MD" \
      "All plan tasks are complete. Review the rings codebase for architectural correctness and code quality. Follow the instructions in REVIEW_PROMPT.md.")

    echo "$review_output"

    if echo "$review_output" | grep -q "^RINGS_DONE"; then
      log "Review signals completion. $iteration build iteration(s), $review_pass review pass(es)."
      log "Results in: $PROJECT_ROOT/src/"
      log "Token usage log: $USAGE_LOG"
      print_usage_summary
      break
    fi

    if echo "$review_output" | grep -q "^REVIEW PASS COMPLETE — issues found"; then
      log "Review pass $review_pass found issues — looping back to build phase."
      continue
    fi

    if has_unchecked_tasks; then
      log "Review pass $review_pass added new tasks (no signal emitted) — looping back to build phase."
      continue
    fi

    log "WARNING: Review pass $review_pass produced no new tasks and no completion signal."
    log "Treating as complete. Check review output above."
    print_usage_summary
    break
  done
}

main "$@"
