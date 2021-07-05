use crate::simulation_state::CombatState;
use std::path::PathBuf;

/// Basically a module for me to mess around writing experimental things without committing to them having a real interface. If I develop something in here for too long, I should make a new, different module for it.

pub fn run(root_path: PathBuf) {
  if let Ok(file) = std::fs::File::open(root_path.join("data/hexaghost.json")) {
    if let Ok(state) = serde_json::from_reader(std::io::BufReader::new(file)) {
      combat_sandbox(state)
    }
  }
}

pub fn combat_sandbox(state: CombatState) {
  println!("{}", state);
  println!("{}", state);
}
