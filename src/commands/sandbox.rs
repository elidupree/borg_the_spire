use crate::ai_utils::Strategy;
use crate::seed_system::{Seed, SingleSeed};
use crate::seeds_concrete::CombatChoiceLineagesKind;
use crate::simulation::{Runner, StandardRunner};
use crate::simulation_state::CombatState;
use crate::start_and_strategy_ai::PurelyRandomStrategy;
use rand::SeedableRng;
use rand_pcg::Pcg64Mcg;
use std::path::PathBuf;

/// Basically a module for me to mess around writing experimental things without committing to them having a real interface. If I develop something in here for too long, I should make a new, different module for it.

pub fn run(root_path: PathBuf) {
  if let Ok(file) = std::fs::File::open(root_path.join("data/hexaghost.json")) {
    if let Ok(state) = serde_json::from_reader(std::io::BufReader::new(file)) {
      combat_sandbox(state)
    }
  }
}

pub fn play_some<S: Strategy>(runner: &mut impl Runner, strategy: &S) {
  runner.run_until_unable();
  while runner.state().turn_number < 3 && !runner.state().combat_over() {
    let choices = strategy.choose_choice(runner.state());
    for choice in choices {
      runner.apply_choice(&choice);
    }
  }
}

pub fn combat_sandbox(state: CombatState) {
  println!("{}", state);
  for _ in 0..3 {
    let seed = SingleSeed::<CombatChoiceLineagesKind>::new(&mut Pcg64Mcg::from_entropy());
    for _ in 0..3 {
      let mut state = state.clone();
      play_some(
        &mut StandardRunner::new(&mut state, seed.view()),
        &PurelyRandomStrategy,
      );
      println!("{}", state);
    }
  }
}
