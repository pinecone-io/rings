use rings::template::{render_prompt, TemplateVars};

#[test]
fn substitutes_all_variables() {
    let vars = TemplateVars {
        phase_name: "builder".to_string(),
        cycle: 2,
        max_cycles: Some(10),
        run: 5,
        iteration: 1,
        runs_per_cycle: 3,
        cost_so_far_usd: 0.142,
    };
    let template = "Phase: {{phase_name}}, cycle {{cycle}}/{{max_cycles}}, run {{run}}, iter {{iteration}}/{{runs_per_cycle}}, cost ${{cost_so_far_usd}}";
    let rendered = render_prompt(template, &vars);
    assert_eq!(
        rendered,
        "Phase: builder, cycle 2/10, run 5, iter 1/3, cost $0.142"
    );
}

#[test]
fn leaves_unknown_variables_intact() {
    let vars = TemplateVars {
        phase_name: "x".to_string(),
        cycle: 1,
        max_cycles: Some(1),
        run: 1,
        iteration: 1,
        runs_per_cycle: 1,
        cost_so_far_usd: 0.0,
    };
    let template = "{{phase_name}} {{unknown_var}}";
    let rendered = render_prompt(template, &vars);
    assert_eq!(rendered, "x {{unknown_var}}");
}

#[test]
fn cost_formats_to_three_decimal_places() {
    let vars = TemplateVars {
        phase_name: "x".to_string(),
        cycle: 1,
        max_cycles: Some(1),
        run: 1,
        iteration: 1,
        runs_per_cycle: 1,
        cost_so_far_usd: 1.0 / 3.0,
    };
    let rendered = render_prompt("{{cost_so_far_usd}}", &vars);
    assert_eq!(rendered, "0.333");
}

#[test]
fn renders_unlimited_max_cycles() {
    let vars = TemplateVars {
        phase_name: "x".to_string(),
        cycle: 1,
        max_cycles: None,
        run: 1,
        iteration: 1,
        runs_per_cycle: 1,
        cost_so_far_usd: 0.0,
    };
    assert_eq!(render_prompt("{{max_cycles}}", &vars), "unlimited");
}
