#!/usr/bin/env bash
# watch-cost.sh — stream cost per invocation from usage.jsonl as it grows
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
USAGE_LOG="$SCRIPT_DIR/usage.jsonl"

tail -f "$USAGE_LOG" | python3 -u -c "
import sys, json

# Print existing lines first (tail -f replays them)
total = 0
for line in sys.stdin:
    line = line.strip()
    if not line:
        continue
    r = json.loads(line)
    cost = r.get('total_cost_usd', 0)
    total += cost
    print(f\"{r['phase']:10}  {r['label']:14}  \${cost:.4f}   running total: \${total:.4f}\")
    sys.stdout.flush()
"
