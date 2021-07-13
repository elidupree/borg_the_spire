pub mod html_views;
pub mod rocket_glue;

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use self::rocket_glue::MessageFromFrontend;
use crate::simulation_state::*;
use crate::start_and_strategy_ai::*;

#[derive(Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Debug, Default)]
pub struct FrontendState {}

pub struct ServerConstants {
  data_files: PathBuf,
}

#[derive(Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Debug, Default)]
pub struct ServerPersistentState {
  frontend_state: FrontendState,
}

pub struct ServerState {
  constants: Arc<ServerConstants>,
  persistent_state: ServerPersistentState,
  inputs: Vec<MessageFromFrontend>,
  combat_state: Option<CombatState>,
  search_state: Option<SearchState>,
  debug_log: String,
}

impl ServerState {
  pub fn set_state(&mut self, state: CombatState) {
    if self.combat_state.as_ref() != Some(&state) {
      self.combat_state = Some(state.clone());
      // let mut playout_state = state.clone();
      self.search_state = Some(SearchState::new(state));
      // let mut runner = StandardRunner::new(
      //   &mut playout_state,
      //   TrivialSeed::new(Pcg64Mcg::from_entropy()),
      //   true,
      // );
      // play_out(&mut runner, &SomethingStrategy {});
      // self.debug_log = runner.debug_log().to_string();
    }
  }
  pub fn change_persistent_state(&mut self, change: impl FnOnce(&mut ServerPersistentState)) {
    // Two small flaws here: it's nonatomic and it does file i/o even though
    // it's called while the client is waiting for a response from the server.
    // Those could be improved, but it's not very important.
    change(&mut self.persistent_state);
    if let Ok(file) = std::fs::File::create(
      &self
        .constants
        .data_files
        .join("server_persistent_state.json"),
    ) {
      let _ = serde_json::to_writer_pretty(std::io::BufWriter::new(file), &self.persistent_state);
    }
  }
}

pub fn processing_thread(state_file: PathBuf, application_state: Arc<Mutex<ServerState>>) {
  // TODO : this isn't the most efficient file watcher system, figure out what is?
  let mut last_modified = None;
  let mut last_check = Instant::now();
  loop {
    let mut guard = application_state.lock();
    // If the state file has been modified, update it.
    if last_check.elapsed() > Duration::from_millis(200) {
      last_check = Instant::now();
      if let Ok(modified) = fs::metadata(&state_file).and_then(|m| m.modified()) {
        if Some(modified) != last_modified {
          last_modified = Some(modified);
          if let Ok(file) = std::fs::File::open(&state_file) {
            if let Ok(state) = serde_json::from_reader(std::io::BufReader::new(file)) {
              guard.set_state(state);
            }
          }
        }
      }
    }
    if let Some(search_state) = &mut guard.search_state {
      if search_state.visits < 2_000_000_000 {
        search_state.search_step();
      }
    }
    //application_state.lock().placeholder += 1;
    else {
      std::mem::drop(guard);
      std::thread::sleep(Duration::from_millis(100));
    }
  }
}

pub fn run(
  static_files: PathBuf,
  data_files: PathBuf,
  state_file: PathBuf,
  address: &str,
  port: u16,
) {
  let mut frontend_state = Default::default();
  if let Ok(file) = std::fs::File::open(&data_files.join("server_persistent_state.json")) {
    if let Ok(state) = serde_json::from_reader(std::io::BufReader::new(file)) {
      frontend_state = state;
    }
  }
  let mut server_state = ServerState {
    constants: Arc::new(ServerConstants { data_files }),
    persistent_state: ServerPersistentState { frontend_state },
    inputs: Vec::new(),
    combat_state: None,
    search_state: None,
    debug_log: String::new(),
  };

  if let Ok(file) = std::fs::File::open(&state_file) {
    if let Ok(state) = serde_json::from_reader(std::io::BufReader::new(file)) {
      server_state.set_state(state);
    }
  }

  let server_state = Arc::new(Mutex::new(server_state));

  std::thread::spawn({
    let server_state = server_state.clone();
    move || {
      processing_thread(state_file, server_state);
    }
  });

  rocket_glue::launch(server_state, static_files, address, port);
}
