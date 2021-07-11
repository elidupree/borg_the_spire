use parking_lot::Mutex;
use rand_pcg::Pcg64Mcg;
use rocket::config::{Config, Environment};
use rocket::response::NamedFile;
use rocket::State;
use rocket_contrib::json::Json;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use typed_html::dom::DOMTree;
use typed_html::elements::FlowContent;
use typed_html::{html, text};

use crate::ai_utils::play_out;
use crate::seed_system::TrivialSeed;
use crate::simulation::*;
use crate::simulation_state::*;
use crate::start_and_strategy_ai::*;
use rand::SeedableRng;
use rocket_contrib::serve::StaticFiles;
use std::fs;

pub type Element = Box<dyn FlowContent<String>>;

impl CombatState {
  pub fn view(&self) -> Element {
    let monsters = self
      .monsters
      .iter()
      .filter(|monster| !monster.gone)
      .map(|monster| {
        html! {
          <div class="monster">
            {text! ("{}", monster)}
          </div>
        }
      });
    let hand = self.hand.iter().map(|card| {
      html! {
        <div class="card">
          {text! ("{}", card)}
        </div>
      }
    });
    html! {
      <div class="combat-state">
        <div class="player">
          {text! ("{}", self.player)}
        </div>
        <div class="monsters">
          {monsters}
        </div>
        <div class="hand">
          {hand}
        </div>
      </div>
    }
  }
}

impl SearchState {
  pub fn view(&self) -> Element {
    let starting_points = self.starting_points.iter().map(|start| {
      let scores = start.candidate_strategies.iter().map(|strategy| {
        {
          text!(
            "average score {:.6} ({} visits)",
            strategy.total_score / strategy.visits as f64,
            strategy.visits
          )
        }
      });
      let mut hypothetical_evaluated_state = start.state.clone();
      //let next_turn = start.state.turn_number + 1;
      let mut runner = StandardRunner::new(
        &mut hypothetical_evaluated_state,
        TrivialSeed::new(Pcg64Mcg::from_entropy()),
        true,
      );
      runner.run_until_unable();
      //let log = runner.debug_log().to_string();
      html! {
        <div class="starting-point">
          <div class="starting-point-heading">
            {text! ("{} visits\n{:?}", start.visits, start.choices)}
            {start.state.view()}
            {hypothetical_evaluated_state.view()}
            //<pre>{text! (log)}</pre>
          </div>
          <div class="strategies">
            {scores}
          </div>
        </div>
      }
    });

    html! {
      <div class="search-state">
        <div class="search-state-heading">
          {text! ("{} visits", self.visits)}
        </div>
        <div class="starting-points">
          {starting_points}
        </div>
      </div>
    }
  }
}

#[derive(Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Debug)]
pub struct InterfaceState {}

pub struct ApplicationState {
  combat_state: Option<CombatState>,
  search_state: Option<SearchState>,
  debug_log: String,
}

impl ApplicationState {
  pub fn set_state(&mut self, state: CombatState) {
    if self.combat_state.as_ref() != Some(&state) {
      self.combat_state = Some(state.clone());
      let mut playout_state = state.clone();
      self.search_state = Some(SearchState::new(state));
      let mut runner = StandardRunner::new(
        &mut playout_state,
        TrivialSeed::new(Pcg64Mcg::from_entropy()),
        true,
      );
      play_out(&mut runner, &SomethingStrategy {});
      self.debug_log = runner.debug_log().to_string();
    }
  }
}

pub struct RocketState {
  application_state: Arc<Mutex<ApplicationState>>,
  static_files: PathBuf,
}

#[allow(unused)]
#[post("/content", data = "<interface_state>")]
fn content(interface_state: Json<InterfaceState>, rocket_state: State<RocketState>) -> String {
  let application_state = rocket_state.application_state.lock();

  let state_representation = application_state
    .search_state
    .as_ref()
    .map(|search_state| search_state.view());
  let document: DOMTree<String> = html! {
    <div id="content">
      {state_representation}
      <pre>{text! (&application_state.debug_log)}</pre>
    </div>
  };
  document.to_string()
}

#[get("/default_interface_state")]
fn default_interface_state() -> Json<InterfaceState> {
  Json(InterfaceState {
    //client_placeholder: 3,
    //placeholder_i32: 5,
    //placeholder_string: "whatever".to_string()
  })
}

#[get("/")]
fn index(rocket_state: State<RocketState>) -> Option<NamedFile> {
  NamedFile::open(rocket_state.static_files.join("index.html")).ok()
}

pub fn processing_thread(state_file: PathBuf, application_state: Arc<Mutex<ApplicationState>>) {
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

pub fn run(static_files: PathBuf, state_file: PathBuf, address: &str, port: u16) {
  let mut application_state = ApplicationState {
    combat_state: None,
    search_state: None,
    debug_log: String::new(),
  };

  if let Ok(file) = std::fs::File::open(&state_file) {
    if let Ok(state) = serde_json::from_reader(std::io::BufReader::new(file)) {
      application_state.set_state(state);
    }
  }

  let application_state = Arc::new(Mutex::new(application_state));

  std::thread::spawn({
    let application_state = application_state.clone();
    move || {
      processing_thread(state_file, application_state);
    }
  });

  rocket::custom(
    Config::build(Environment::Development)
      .address(address)
      .port(port)
      //.log_level(LoggingLevel::Off)
      .unwrap(),
  )
  .mount("/media/", StaticFiles::from(static_files.join("media")))
  .mount("/", routes![index, content, default_interface_state])
  .manage(RocketState {
    application_state,
    static_files,
  })
  .launch();
}
