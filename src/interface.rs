use std::path::PathBuf;
use std::sync::Arc;
use std::io::BufRead;
use std::time::Duration;
use std::ops::Add;
use serde::{Serialize, Deserialize};
use parking_lot::Mutex;
use rocket::State;
use rocket::config::{Config, Environment, LoggingLevel};
use rocket::response::NamedFile;
//use rocket::response::content::Json;
use rocket_contrib::json::Json;
use typed_html::{html, text};
use typed_html::dom::DOMTree ;
use typed_html::elements:: FlowContent;

use crate::communication_mod_state;
use crate::simulation_state::*;
use crate::simulation::*;
use crate::mcts::*;
use crate::start_and_strategy_ai::*;

pub type Element = Box <dyn FlowContent <String>>;

impl CombatState {
  pub fn view (&self)->Element {
    let monsters =self.monsters.iter().filter (| monster |!monster.gone).map (| monster | {
            html! {
              <div class="monster">
                {text! ("{:?} i{} {:?}", monster.monster_id, monster.intent(), monster.creature)}
              </div>
            }
          });
    let hand =self.hand.iter().map (| card | {
            html! {
              <div class="card">
                {text! ("{:?}", card)}
              </div>
            }
          });
    html! {
      <div class="combat-state">
        <div class="player">
          {text! ("({}) {:?}", self.player.energy, self.player.creature)}
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

#[derive (Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Debug, Default)]
pub struct NodeIdentifier {
  pub choice_choices: Vec<Choice>,
  pub continuation_choices: Vec<Replay>,
}

impl Add <Choice> for NodeIdentifier {
  type Output = Self;
  fn add (self, other: Choice)->NodeIdentifier {
    let mut result = self.clone();
    result.choice_choices.push (other);
    result
  }
}


impl Add <Replay> for NodeIdentifier {
  type Output = Self;
  fn add (self, other: Replay)->NodeIdentifier {
    let mut result = self.clone();
    result.continuation_choices.push (other);
    result
  }
}

impl ChoiceNode {
  pub fn view (&self, state: & CombatState, my_id: NodeIdentifier, viewed_id: &NodeIdentifier)->Element {
    let choices = if let Some(choice) = viewed_id.choice_choices.get (my_id.choice_choices.len()) {
      if let Some ((_, results)) = self.choices.iter().find (| (a,_results) | a == choice) {
        vec![results.view (state, choice, my_id + choice.clone(), viewed_id)]
      }
      else {Vec::new()}
    }
    else if viewed_id.choice_choices.len() + 1 > my_id.choice_choices.len() {
      self.choices.iter().filter (| (_, results) | results.visits >0).map (| (choice, results) |
        results.view (state, choice, my_id.clone() + choice.clone(), viewed_id)
      ).collect()
    }
    else {Vec::new()};
    
    html! {
      <div class="choice-node">
        <div class="choice-node-heading">
          {text! ("Average score {:.6} ({} visits)", self.total_score/self.visits as f64, self.visits)}
        </div>
        {state.view()}
        <div class="choices">
          {choices}
        </div>
      </div>
    }

  }
}

impl ChoiceResults {
  pub fn view (&self, state: & CombatState, choice: & Choice, my_id: NodeIdentifier, viewed_id: &NodeIdentifier)->Element {
    
    let continuations = if let Some(replay) = viewed_id. continuation_choices.get (my_id. continuation_choices.len()) {
      if let Some (node) = self.continuations.get (replay) {
        vec![node.view (& state.after_replay (choice, replay), my_id + replay.clone(), viewed_id)]
      }
      else {Vec::new()}
    }
    else if viewed_id. continuation_choices.len() + 1 > my_id. continuation_choices.len() {
      self.continuations.iter().filter (| (_, node) | node.visits >0).map (| (replay, node) | 
        node.view (& state.after_replay (choice, replay), my_id.clone() + replay.clone(), viewed_id)
        ).collect()
    }
    else {Vec::new()};
    
    html! {
      <div class="choice-node">
        <div class="choice-node-heading">
          {text! ("{:?}: average score {:.6} ({} visits)", choice, self.total_score/self.visits as f64, self.visits)}
        </div>
        <div class="continuations">
          {continuations}
        </div>
      </div>
    }
  }
}

impl SearchState {
  pub fn view (&self)->Element {
    let starting_points = self.starting_points.iter().map (| start | {
      let scores = start.candidate_strategies.iter().map (| strategy | {
        {text! ("average score {:.6} ({} visits)", strategy.total_score/strategy.visits as f64, strategy.visits)}
      });
      html! {
        <div class="starting-point">
          <div class="starting-point-heading">
            {text! ("{} visits\n{:?}", start.visits, start.choices)}
            {start.state.view()}
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

#[derive (Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Debug)]
pub struct InterfaceState {
  viewed_node: NodeIdentifier
}

pub struct ApplicationState {
  combat_state: Option <CombatState>,
  search_tree: Option <SearchTree>,
  search_state: Option <SearchState>,
}

impl ApplicationState {
  pub fn set_state (&mut self, state: CombatState) {
    self.combat_state = Some (state.clone());
                //self.search_tree = Some (SearchTree::new (state)) ;
                self.search_state = Some (SearchState::new (state)) ;
  }
}

pub struct RocketState {
  application_state: Arc <Mutex <ApplicationState>>,
  root_path: PathBuf,
}

#[post ("/content", data = "<interface_state>")]
fn content (interface_state: Json <InterfaceState>, rocket_state: State <RocketState>)->String {
  let tree_representation = rocket_state.application_state.lock().search_tree.as_ref().map (| search_tree | search_tree.root.view(& search_tree.initial_state, NodeIdentifier::default(), & interface_state.viewed_node));
  let state_representation = rocket_state.application_state.lock().search_state.as_ref().map (| search_state | search_state.view());
  let document: DOMTree <String> = html! {
    <div id="content">
      {tree_representation}
      {state_representation}
    </div>
  };
  document.to_string()
  
}

#[get ("/default_interface_state")]
fn default_interface_state ()->Json <InterfaceState> {
  Json(InterfaceState {
    viewed_node: NodeIdentifier::default(),
    //client_placeholder: 3,
    //placeholder_i32: 5,
    //placeholder_string: "whatever".to_string()
  })
}

#[get ("/")]
fn index (rocket_state: State <RocketState>)->Option <NamedFile> {
  NamedFile::open (rocket_state.root_path.join ("static/index.html")).ok()
}

#[get ("/media/<file..>")]
fn media (file: PathBuf, rocket_state: State <RocketState>)->Option <NamedFile> {
  NamedFile::open (rocket_state.root_path.join ("static/media/").join (file)).ok()
}

pub fn communication_thread (root_path: PathBuf, application_state: Arc <Mutex <ApplicationState>>) {
  let input = std::io::stdin();
  let input = input.lock();
  let mut failed = false;

  for line in input.lines() {
    let line = line.unwrap();
    if line.len() > 3 {
      let interpreted: Result<communication_mod_state::CommunicationState, _> =
        serde_json::from_str(& line);
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
            if let Ok (file) = std::fs::File::create (root_path.join ("last_state.json")) {
    let _= serde_json::to_writer_pretty (std::io::BufWriter::new (file), & state);    
  }
            let mut lock = application_state.lock();
            lock.set_state (state);
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

pub fn processing_thread (application_state: Arc <Mutex <ApplicationState>>) {
  loop {
    let mut guard = application_state.lock();
    if let Some (search_tree) = &mut guard.search_tree {
      if search_tree.root.visits < 2_000_000 {
      for _ in 0..10 {
                search_tree.search_step();
              }
              }
    }
    else if let Some (search_state) = &mut guard.search_state {
      if search_state.visits < 2_000_000_000 {
      search_state.search_step();
              }
    }
    //application_state.lock().placeholder += 1;
    else {
      std::mem::drop (guard);
      std::thread::sleep (Duration::from_millis (100));
}
  }
}

pub fn run(root_path: PathBuf) {
  let mut application_state = ApplicationState {combat_state: None, search_tree: None, search_state: None};
  
  if let Ok (file) = std::fs::File::open (root_path.join ("last_state.json")) {
    if let Ok (state) = serde_json::from_reader (std::io::BufReader::new (file)) {
      application_state.set_state (state);
    }
    
  }
  
  let application_state = Arc::new (Mutex::new (application_state)) ;
  
  std::thread::spawn ({let root_path = root_path.clone(); let application_state = application_state.clone(); move | | {
    communication_thread (root_path, application_state);
  }});
  
  std::thread::spawn ({let application_state = application_state.clone(); move | | {
    processing_thread (application_state);
  }});

  rocket::custom (Config::build (Environment::Development).address ("localhost").port (3508).log_level (LoggingLevel::Off).unwrap())
  .mount ("/", routes! [index, media, content, default_interface_state])
  .manage (RocketState {application_state, root_path})
  .launch();
}
