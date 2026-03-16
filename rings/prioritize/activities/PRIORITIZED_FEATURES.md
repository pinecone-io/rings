# Prioritized Feature Queue

Features are listed in implementation priority order. The top of this list is what the next
`/replan` batch should draw from.

**Status values in this file:**
- Features listed here have status `PRIORITIZED` in `specs/feature_inventory.md`
- When a feature moves into a `/replan` batch it becomes `PLANNED`
- When implemented and tested it becomes `COMPLETE`

**How to use:**
1. Run `rings run rings/prioritize/prioritize.rings.toml` to populate this queue (or extend it)
2. When starting a new implementation batch, run `/replan` and use the top N entries here
   as the candidate pool (rather than re-running the full voting wave)
3. After implementation, mark features `COMPLETE` in `specs/feature_inventory.md`

---

<!-- Election cycles append entries below this line -->
