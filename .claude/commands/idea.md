Capture a rough idea and append it to queues/IDEAS.md without evaluating or processing it.

## Steps

1. The user's idea is whatever they wrote after `/idea`. If no text was provided, ask them to describe the idea in one or a few sentences.
2. If `queues/IDEAS.md` does not exist at the project root, create it with a minimal header:
   ```markdown
   # Ideas

   Rough notes and unprocessed ideas. Run `/process-ideas` to evaluate these against the specs.
   ```
3. Append the idea as a new bullet under a `## Unprocessed` section (create that section if it doesn't exist), including today's date:
   ```markdown
   - [YYYY-MM-DD] <idea text>
   ```
4. Confirm to the user that the idea was saved and remind them they can run `/process-ideas` when ready to evaluate it.

Do not interpret, refine, or evaluate the idea — record it exactly as the user expressed it.
