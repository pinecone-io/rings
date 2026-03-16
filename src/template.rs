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

pub fn render_prompt(template: &str, vars: &TemplateVars) -> String {
    let max_cycles_str = vars
        .max_cycles
        .map(|n| n.to_string())
        .unwrap_or_else(|| "unlimited".to_string());
    template
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
        .replace("{{context_dir}}", &vars.context_dir)
}
