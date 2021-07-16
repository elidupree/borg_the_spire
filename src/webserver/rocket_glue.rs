use crate::webserver::ServerSharedState;
use parking_lot::Mutex;
use rocket::config::Environment;
use rocket::response::NamedFile;
use rocket::{Config, State};
use rocket_contrib::json::Json;
use rocket_contrib::serve::StaticFiles;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;

struct RocketState {
  server_state: Arc<Mutex<ServerSharedState>>,
  static_files: PathBuf,
}

#[post("/content")]
fn content(rocket_state: State<RocketState>) -> String {
  let server_state = rocket_state.server_state.lock();
  server_state.html_string.clone()
}

#[derive(Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Debug)]
pub enum MessageFromFrontend {}

#[allow(clippy::unit_arg)]
// why is this needed? no idea, probably rocket proc macro stuff
#[post("/input", data = "<input>")]
fn input(input: Json<MessageFromFrontend>, rocket_state: State<RocketState>) {
  let Json(input) = input;

  rocket_state.server_state.lock().inputs.push(input);
}

#[get("/")]
fn index(rocket_state: State<RocketState>) -> Option<NamedFile> {
  NamedFile::open(rocket_state.static_files.join("index.html")).ok()
}

pub fn launch(
  server_state: Arc<Mutex<ServerSharedState>>,
  static_files: PathBuf,
  address: &str,
  port: u16,
) {
  rocket::custom(
    Config::build(Environment::Development)
      .address(address)
      .port(port)
      //.log_level(LoggingLevel::Off)
      .unwrap(),
  )
  .mount("/media/", StaticFiles::from(static_files.join("media")))
  .mount("/", routes![index, content, input])
  .manage(RocketState {
    server_state,
    static_files,
  })
  .launch();
}
