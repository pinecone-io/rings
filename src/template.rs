use lazy_static::lazy_static;
use regex::Regex;
use std::collections::HashSet;

lazy_static! {
    // Regex to find template variables: {{variable_name}}
    static ref RE_TEMPLATE_VAR: Regex = Regex::new(r"\{\{([^{}]+)\}\}").unwrap();
}

// Known template variables (9 total: 7 from spec + workflow_name + context_dir)
pub const KNOWN_VARS: &[&str] = &[
    "phase_name",
    "cycle",
    "max_cycles",
    "iteration",
    "runs_per_cycle",
    "run",
    "cost_so_far_usd",
    "workflow_name",
    "context_dir",
];

pub struct TemplateVars {
    pub phase_name: String,
    pub cycle: u32,
    pub max_cycles: Option<u32>, // None = unlimited
    pub run: u32,
    pub iteration: u32,
    pub runs_per_cycle: u32,
    pub cost_so_far_usd: f64,
    pub workflow_name: String,
    pub context_dir: String,
}

/// Find unknown template variables in a template string.
/// Returns a deduplicated list of variable names that are not in the known list.
pub fn find_unknown_variables(template: &str, known: &[&str]) -> Vec<String> {
    // Step 1: Protect {{{{ and }}}} escapes with sentinels
    let open_sentinel = "\x00ESCAPE_OPEN\x00";
    let close_sentinel = "\x00ESCAPE_CLOSE\x00";
    let protected = template
        .replace("{{{{", open_sentinel)
        .replace("}}}}", close_sentinel);

    // Step 2: Find all template variables
    let mut unknown = HashSet::new();
    let known_set: HashSet<&str> = known.iter().copied().collect();

    for cap in RE_TEMPLATE_VAR.captures_iter(&protected) {
        if let Some(var_match) = cap.get(1) {
            let var_name = var_match.as_str().trim();
            if !known_set.contains(var_name) {
                unknown.insert(var_name.to_string());
            }
        }
    }

    // Step 3: Return sorted deduplicated list for deterministic output
    let mut result: Vec<String> = unknown.into_iter().collect();
    result.sort();
    result
}

pub fn render_prompt(template: &str, vars: &TemplateVars) -> String {
    // Step 1: Protect {{{{ and }}}} escape sequences with sentinels
    let open_sentinel = "\x00ESCAPE_OPEN\x00";
    let close_sentinel = "\x00ESCAPE_CLOSE\x00";
    let protected = template
        .replace("{{{{", open_sentinel)
        .replace("}}}}", close_sentinel);

    // Step 2: Apply variable substitutions
    let max_cycles_str = vars
        .max_cycles
        .map(|n| n.to_string())
        .unwrap_or_else(|| "unlimited".to_string());
    let result = protected
        .replace("{{phase_name}}", &vars.phase_name)
        .replace("{{cycle}}", &vars.cycle.to_string())
        .replace("{{max_cycles}}", &max_cycles_str)
        .replace("{{run}}", &vars.run.to_string())
        .replace("{{iteration}}", &vars.iteration.to_string())
        .replace("{{runs_per_cycle}}", &vars.runs_per_cycle.to_string())
        .replace(
            "{{cost_so_far_usd}}",
            &format!("{:.3}", vars.cost_so_far_usd),
        )
        .replace("{{workflow_name}}", &vars.workflow_name)
        .replace("{{context_dir}}", &vars.context_dir);

    // Step 3: Restore {{{{ and }}}} from sentinels
    result
        .replace(open_sentinel, "{{")
        .replace(close_sentinel, "}}")
}
