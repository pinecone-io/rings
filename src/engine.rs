// Stub for engine module - scheduling iterator will be implemented below
use crate::workflow::PhaseConfig;

#[derive(Debug, Clone)]
pub struct RunSpec {
    pub global_run_number: u32,
    pub cycle: u32,
    pub phase_name: String,
    pub phase_index: usize,
    pub phase_iteration: u32,
    pub phase_total_iterations: u32,
    /// Resolved prompt text (after template substitution, if needed — populated by engine)
    pub prompt_text: Option<String>,
}

pub struct RunSchedule<'a> {
    phases: &'a [PhaseConfig],
    max_cycles: u32,
    current_cycle: u32,
    current_phase_index: usize,
    current_iteration: u32,
    global_run_number: u32,
}

impl<'a> RunSchedule<'a> {
    pub fn new(phases: &'a [PhaseConfig], max_cycles: u32) -> Self {
        Self {
            phases,
            max_cycles,
            current_cycle: 1,
            current_phase_index: 0,
            current_iteration: 1,
            global_run_number: 1,
        }
    }

    /// Advance past `last_completed_run` already-completed runs.
    pub fn resume_from(
        phases: &'a [PhaseConfig],
        max_cycles: u32,
        last_completed_run: u32,
    ) -> Self {
        let mut schedule = Self::new(phases, max_cycles);
        for _ in 0..last_completed_run {
            if schedule.next().is_none() {
                break;
            }
        }
        schedule
    }
}

impl<'a> Iterator for RunSchedule<'a> {
    type Item = RunSpec;

    fn next(&mut self) -> Option<RunSpec> {
        if self.current_cycle > self.max_cycles {
            return None;
        }
        if self.current_phase_index >= self.phases.len() {
            return None;
        }

        let phase = &self.phases[self.current_phase_index];
        let spec = RunSpec {
            global_run_number: self.global_run_number,
            cycle: self.current_cycle,
            phase_name: phase.name.clone(),
            phase_index: self.current_phase_index,
            phase_iteration: self.current_iteration,
            phase_total_iterations: phase.runs_per_cycle,
            prompt_text: None,
        };

        self.global_run_number += 1;
        self.current_iteration += 1;

        if self.current_iteration > phase.runs_per_cycle {
            self.current_iteration = 1;
            self.current_phase_index += 1;
            if self.current_phase_index >= self.phases.len() {
                self.current_phase_index = 0;
                self.current_cycle += 1;
            }
        }

        Some(spec)
    }
}
