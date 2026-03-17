## Unprocessed

[~] RINGS_CONTINUE signal: a per-cycle short-circuit analogous to `continue` in a loop. when a phase emits a line containing RINGS_CONTINUE, rings skips all remaining phases in the current cycle and immediately begins the next cycle. useful when an early phase determines the current cycle has nothing to do (e.g. an idea is out-of-scope and needs no review or write phases). configured as continue_signal in the workflow TOML, similar to completion_signal.

map/reduce phase mode: a first-class parallel execution primitive for workflows that need to fan out to N independent workers and collect their results. a phase with mode = "map" runs N times (configured via a `workers` field), each run receiving a {{worker_index}} template variable to differentiate its work (e.g. selecting a persona). rings manages per-worker log files and cost tracking individually. a subsequent phase with mode = "reduce" has named access to all worker outputs from the preceding map phase without requiring manual file coordination. this makes patterns like review panels, parallel validators, and multi-perspective analysis native to the rings model rather than requiring opaque Claude subagent dispatch inside a single invocation.

It would be interesting to think about if there is a way to implement scheduling for situations where I do want to iterate in cycles but over longer durations of time. For example, if I had a workflow processing bug reports from github issues then it would probably be considered an abuse of github to poll their issues endpoint with a high frequency. Instead, I could have a rings workflow wake up once per day and run a cycle over all the new bug reports.

there should be some sort of agent-help command or something that can emit instructions on how to approach different aspects of working with rings. The help should be progressively disclosed rather than dumping a ton fo stuff into context at once. Perhaps agent-help with no arguments prints docs explaining different topics that have help associated. E.g. something like "rings agent-help --new-workflow" or "rings agent-help --validate-workflow" or "rings agent-help --token-efficiency" or "rings agent-help --scheduling".

a phase id and workflow idea tracked in config seems useful in order to track and report usage in a way that is easier to understand. probably prompts should be pre-pended with relevant information about where you are in a workflow. e.g. instead of every individual prompt having to self-identify, rings should handle setting up that frontmatter e.g. "You are the *triage-phase* of the *bug-fixing* rings workflow."

file paths in the toml should be able to resolve paths whether they are absolute or relative to the toml file location. if a path is ambiguous because both locations contain files, then the user should get some kind of feedback

it seems like there could be a collision if multiple rings instances are running at the same time. is .rings.lock a problem?
