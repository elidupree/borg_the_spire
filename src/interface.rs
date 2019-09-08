use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::io::BufRead;
use std::time::Duration;
use serde::{Serialize, Deserialize};
use rocket::State;
use rocket::config::{Config, Environment, LoggingLevel};
use rocket::response::NamedFile;
//use rocket::response::content::Json;
use rocket_contrib::json::Json;
use typed_html::{html, text};
use typed_html::dom::DOMTree ;

use crate::{simulation_state, communication_mod_state};

#[derive (Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Debug)]
pub struct InterfaceState {
  client_placeholder: usize,
  //placeholder_i32: i32,
  //placeholder_string: String,
}

pub struct ApplicationState {
  combat_state: Option <simulation_state::CombatState>,
}

pub struct RocketState {
  application_state: Arc <Mutex <ApplicationState>>,
  root_path: PathBuf,
}

#[post ("/content", data = "<interface_state>")]
fn content (interface_state: Json <InterfaceState>, rocket_state: State <RocketState>)->String {
  let state_representation = format! ("{:?}", rocket_state.application_state.lock().unwrap().combat_state);
  let document: DOMTree <String> = html! {
    <div>
      {text!(state_representation)}
    </div>
  };
  document.to_string()
  
}

#[get ("/default_interface_state")]
fn default_interface_state ()->Json <InterfaceState> {
  Json(InterfaceState {
    client_placeholder: 3,
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

pub fn communication_thread (application_state: Arc <Mutex <ApplicationState>>) {
  let input = std::io::stdin();
  let mut input = input.lock();
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
            simulation_state::CombatState::from_communication_mod(game_state, None)
          });
          if let Some(state) = state {
            
            eprintln!("combat happening:\n{:#?}", state);
            application_state.lock().unwrap().combat_state = Some (state);
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
    //application_state.lock().unwrap().placeholder += 1;
    std::thread::sleep (Duration::from_millis (100));
  }
}

pub fn run(root_path: PathBuf) {
  let application_state = ApplicationState {combat_state: None};
  
  let application_state = Arc::new (Mutex::new (application_state)) ;
  
  std::thread::spawn ({let application_state = application_state.clone(); move | | {
    communication_thread (application_state);
  }});
  
  std::thread::spawn ({let application_state = application_state.clone(); move | | {
    processing_thread (application_state);
  }});

  rocket::custom (Config::build (Environment::Development).address ("localhost").port (3508).log_level (LoggingLevel::Off).unwrap())
  .mount ("/", routes! [index, media, content, default_interface_state])
  .manage (RocketState {application_state, root_path})
  .launch();
}
