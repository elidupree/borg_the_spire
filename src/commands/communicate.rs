use std::io::BufRead;
use std::path::PathBuf;

use crate::communication_mod_state;
use crate::simulation_state::CombatState;

pub fn communicate(state_file: PathBuf) {
  println!("ready");

  let input = std::io::stdin();
  let input = input.lock();
  let mut failed = false;

  for line in input.lines() {
    let line = line.unwrap();
    if line.len() > 3 {
      let interpreted: Result<communication_mod_state::CommunicationState, _> =
        serde_json::from_str(&line);
      match interpreted {
        Ok(state) => {
          eprintln!("received state from communication mod");
          let state = state.game_state.as_ref().and_then(|game_state| {
            eprintln!(
              "player energy: {:?}",
              game_state.combat_state.as_ref().map(|cs| cs.player.energy)
            );
            CombatState::from_communication_mod(game_state, None)
          });
          if let Some(state) = state {
            eprintln!("combat happening:\n{:#?}", state);
            if let Ok(file) = std::fs::File::create(state_file.clone()) {
              let _ = serde_json::to_writer_pretty(std::io::BufWriter::new(file), &state);
            }
          }
        }
        Err(err) => {
          eprintln!("received non-state from communication mod {:?}", err);
          if !failed {
            eprintln!("data: {:?}", line);
          }
          failed = true;
        }
      }
    }
  }
}
