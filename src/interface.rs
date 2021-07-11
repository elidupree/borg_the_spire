use parking_lot::Mutex;
use rand_pcg::Pcg64Mcg;
use rocket::config::{Config, Environment, LoggingLevel};
use rocket::response::NamedFile;
use rocket::State;
use rocket_contrib::json::Json;
use serde::{Deserialize, Serialize};
use std::io::BufRead;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use typed_html::dom::DOMTree;
use typed_html::elements::FlowContent;
use typed_html::{html, text};

use crate::ai_utils::play_out;
use crate::communication_mod_state;
use crate::seed_system::TrivialSeed;
use crate::simulation::*;
use crate::simulation_state::*;
use crate::start_and_strategy_ai::*;
use rand::SeedableRng;

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
      run_until_unable(&mut runner);
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
  root_path: PathBuf,
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
  NamedFile::open(rocket_state.root_path.join("static/index.html")).ok()
}

#[get("/media/<file..>")]
fn media(file: PathBuf, rocket_state: State<RocketState>) -> Option<NamedFile> {
  NamedFile::open(rocket_state.root_path.join("static/media/").join(file)).ok()
}

pub fn communication_thread(root_path: PathBuf, application_state: Arc<Mutex<ApplicationState>>) {
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
            if let Ok(file) = std::fs::File::create(root_path.join("last_state.json")) {
              let _ = serde_json::to_writer_pretty(std::io::BufWriter::new(file), &state);
            }
            let mut lock = application_state.lock();
            lock.set_state(state);
            /*let mut tree = mcts::Tree::new(state);

            let start = Instant::now();
            while Instant::now() - start < Duration::from_millis(1000) {
              for _ in 0..100 {
                tree.search_step();
              }
            }
            tree.print_stuff();*/
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

pub fn processing_thread(application_state: Arc<Mutex<ApplicationState>>) {
  loop {
    let mut guard = application_state.lock();
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

pub fn run(root_path: PathBuf) {
  let mut application_state = ApplicationState {
    combat_state: None,
    search_state: None,
    debug_log: String::new(),
  };

  if let Ok(file) = std::fs::File::open(root_path.join("last_state.json")) {
    if let Ok(state) = serde_json::from_reader(std::io::BufReader::new(file)) {
      application_state.set_state(state);
    }
  }

  let application_state = Arc::new(Mutex::new(application_state));

  std::thread::spawn({
    let root_path = root_path.clone();
    let application_state = application_state.clone();
    move || {
      communication_thread(root_path, application_state);
    }
  });

  std::thread::spawn({
    let application_state = application_state.clone();
    move || {
      processing_thread(application_state);
    }
  });

  rocket::custom(
    Config::build(Environment::Development)
      .address("localhost")
      .port(3509)
      .log_level(LoggingLevel::Off)
      .unwrap(),
  )
  .mount("/", routes![index, media, content, default_interface_state])
  .manage(RocketState {
    application_state,
    root_path,
  })
  .launch();
}
