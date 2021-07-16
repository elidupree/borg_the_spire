pub mod html_views;
pub mod rocket_glue;

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

use self::rocket_glue::MessageFromFrontend;
use crate::analysis_flows::{AnalysisFlows, AnalysisFlowsSpec};
use crate::simulation_state::*;
use typed_html::dom::DOMTree;
use typed_html::html;

pub struct ServerConstants {
  data_files: PathBuf,
  state_file: PathBuf,
}

#[derive(Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Debug)]
pub struct ServerPersistentState {
  analysis_flows_spec_file: PathBuf,
}

impl Default for ServerPersistentState {
  fn default() -> Self {
    ServerPersistentState {
      analysis_flows_spec_file: PathBuf::from("default_components.json"),
    }
  }
}

pub struct ServerSharedState {
  constants: Arc<ServerConstants>,
  persistent_state: ServerPersistentState,
  inputs: Vec<MessageFromFrontend>,
  html_string: String,
}

pub struct ProcessingThreadState {
  constants: Arc<ServerConstants>,
  server_shared: Arc<Mutex<ServerSharedState>>,
  combat_state: Option<CombatState>,
  analysis_flows_spec: Option<AnalysisFlowsSpec>,
  analysis_flows: Option<AnalysisFlows>,
  last_file_check: Instant,
  last_report: Instant,
  last_state_last_modified: Option<SystemTime>,
  analysis_flows_spec_last_modified: Option<(PathBuf, SystemTime)>,
}
impl ServerSharedState {
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

impl ProcessingThreadState {
  pub fn new(server_shared: Arc<Mutex<ServerSharedState>>) -> Self {
    let constants = server_shared.lock().constants.clone();
    ProcessingThreadState {
      constants,
      server_shared,
      combat_state: None,
      analysis_flows_spec: None,
      analysis_flows: None,
      last_file_check: Instant::now(),
      last_report: Instant::now(),
      last_state_last_modified: None,
      analysis_flows_spec_last_modified: None,
    }
  }

  pub fn set_combat_state(&mut self, state: CombatState) {
    if self.combat_state.as_ref() != Some(&state) {
      // let mut playout_state = state.clone();
      if let Some(spec) = &self.analysis_flows_spec {
        self.analysis_flows = Some(AnalysisFlows::new(spec, state.clone()));
      }
      self.combat_state = Some(state);

      // let mut runner = StandardRunner::new(
      //   &mut playout_state,
      //   TrivialSeed::new(Pcg64Mcg::from_entropy()),
      //   true,
      // );
      // play_out(&mut runner, &SomethingStrategy {});
      // self.debug_log = runner.debug_log().to_string();
    }
  }
  pub fn set_analysis_flows_spec(&mut self, spec: AnalysisFlowsSpec) {
    if self.analysis_flows_spec.as_ref() != Some(&spec) {
      if let Some(flows) = &mut self.analysis_flows {
        flows.update_from_spec(&spec);
      } else if let Some(state) = &self.combat_state {
        self.analysis_flows = Some(AnalysisFlows::new(&spec, state.clone()));
      }
      self.analysis_flows_spec = Some(spec);
    }
  }

  pub fn step(&mut self) {
    // If the state file has been modified, update it.
    if self.last_file_check.elapsed() > Duration::from_millis(200) {
      self.last_file_check = Instant::now();

      if let Ok(modified) = fs::metadata(&self.constants.state_file).and_then(|m| m.modified()) {
        if Some(modified) != self.last_state_last_modified {
          self.last_state_last_modified = Some(modified);
          if let Ok(file) = std::fs::File::open(&self.constants.state_file) {
            if let Ok(state) = serde_json::from_reader(std::io::BufReader::new(file)) {
              self.set_combat_state(state);
            }
          }
        }
      }

      let analysis_flows_file = self.constants.data_files.join(
        &self
          .server_shared
          .lock()
          .persistent_state
          .analysis_flows_spec_file,
      );
      if let Ok(modified) = fs::metadata(&analysis_flows_file).and_then(|m| m.modified()) {
        let new_value = Some((analysis_flows_file.clone(), modified));
        if new_value != self.analysis_flows_spec_last_modified {
          self.analysis_flows_spec_last_modified = new_value;
          if let Ok(file) = std::fs::File::open(&analysis_flows_file) {
            match serde_json::from_reader(std::io::BufReader::new(file)) {
              Ok(spec) => {
                self.set_analysis_flows_spec(spec);
              }
              Err(e) => {
                dbg!(e);
              }
            }
          }
        }
      }
    }
    if let Some(flows) = &mut self.analysis_flows {
      flows.step();

      if self.last_report.elapsed() > Duration::from_millis(100) {
        let report: DOMTree<String> = html! {
          <div id="content">
            {flows.html_report()}
          </div>
        };
        let html_string = report.to_string();
        self.server_shared.lock().html_string = html_string;
      }
    } else {
      std::thread::sleep(Duration::from_millis(100));
    }
  }

  pub fn run(&mut self) {
    loop {
      self.step();
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
  let mut persistent_state = Default::default();
  if let Ok(file) = std::fs::File::open(&data_files.join("server_persistent_state.json")) {
    if let Ok(state) = serde_json::from_reader(std::io::BufReader::new(file)) {
      persistent_state = state;
    }
  }
  let server_state = ServerSharedState {
    constants: Arc::new(ServerConstants {
      data_files,
      state_file,
    }),
    persistent_state,
    inputs: Vec::new(),
    html_string: r#"<div id="content">Starting...</div>"#.to_string(),
  };

  let server_state = Arc::new(Mutex::new(server_state));

  std::thread::spawn({
    let server_state = server_state.clone();
    move || {
      ProcessingThreadState::new(server_state).run();
    }
  });

  rocket_glue::launch(server_state, static_files, address, port);
}
