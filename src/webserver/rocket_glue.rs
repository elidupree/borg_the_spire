use crate::webserver::{FrontendState, ServerState};
use parking_lot::Mutex;
use rocket::config::Environment;
use rocket::response::NamedFile;
use rocket::{Config, State};
use rocket_contrib::json::Json;
use rocket_contrib::serve::StaticFiles;
use std::path::PathBuf;
use std::sync::Arc;

struct RocketState {
  server_state: Arc<Mutex<ServerState>>,
  static_files: PathBuf,
}

#[allow(unused)]
#[post("/content", data = "<interface_state>")]
fn content(interface_state: Json<FrontendState>, rocket_state: State<RocketState>) -> String {
  let mut server_state = rocket_state.server_state.lock();
  server_state.change_persistent_state(|p| p.frontend_state = interface_state.0);
  server_state.view().to_string()
}

#[get("/default_interface_state")]
fn default_interface_state() -> Json<FrontendState> {
  Json(FrontendState {
        //client_placeholder: 3,
        //placeholder_i32: 5,
        //placeholder_string: "whatever".to_string()
    })
}

#[get("/")]
fn index(rocket_state: State<RocketState>) -> Option<NamedFile> {
  NamedFile::open(rocket_state.static_files.join("index.html")).ok()
}

pub fn launch(
  server_state: Arc<Mutex<ServerState>>,
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
  .mount("/", routes![index, content, default_interface_state])
  .manage(RocketState {
    server_state,
    static_files,
  })
  .launch();
}
