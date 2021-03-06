use borg_the_spire::competing_optimizers::CompetitorSpecification;
use borg_the_spire::{
  commands::{communicate, sandbox, watch},
  competing_optimizers, webserver,
};
use clap::{App, AppSettings, Arg, SubCommand};
use std::path::PathBuf;

fn main() {
  let matches = App::new("Borg the Spire")
    .version("0.1")
    .author("Eli Dupree <vcs@elidupree.com>")
    .subcommand(
      SubCommand::with_name("communicate")
        .long_about("The command to run as the child process for CommunicationMod. Listens for game states and saves them into the given state-file.")
        .arg(Arg::with_name("state-file").required(true)),
    )
      .subcommand(
        SubCommand::with_name("live-analyze")
            .long_about("Watch and analyze a given state-file, displaying a report you can view in a browers.")
            .arg(Arg::with_name("state-file").long("state-file").required(true).takes_value(true))
            .arg(Arg::with_name("ip").long("ip").required(true).takes_value(true))
            .arg(Arg::with_name("port").long("port").required(true).takes_value(true))
            .arg(Arg::with_name("static-files").long("static-files").required(true).takes_value(true).help("The path to the static html/etc files for BtS, typically `./static`"))
            .arg(Arg::with_name("data-files").long("data-files").required(true).takes_value(true).help("The path to the data files for BtS, typically `./data`")),
      )
      .subcommand(
        SubCommand::with_name("watch")
            .setting(AppSettings::TrailingVarArg)
            .arg(Arg::with_name("executable-original").required(true))
            .arg(Arg::with_name("executable-copy").required(true))
            .arg(Arg::with_name("args").multiple(true)),
      )
    .subcommand(
      SubCommand::with_name("run_competing_optimizers")
        .arg(Arg::with_name("competitor-spec-file").required(true)),
    )
    .subcommand(SubCommand::with_name("sandbox").arg(Arg::with_name("root-path").required(true)))
    .get_matches();

  match matches.subcommand() {
    ("communicate", Some(matches)) => {
      communicate::communicate(PathBuf::from(matches.value_of("state-file").unwrap()))
    }
    ("live-analyze", Some(matches)) => {
      webserver::run(
        PathBuf::from(matches.value_of("static-files").unwrap()),
        PathBuf::from(matches.value_of("data-files").unwrap()),
        PathBuf::from(matches.value_of("state-file").unwrap()),
        matches.value_of("ip").unwrap(),
        matches.value_of("port").unwrap().parse::<u16>().unwrap(),
      );
    }
    ("watch", Some(matches)) => {
      watch::watch(
        matches.value_of("executable-original").unwrap(),
        matches.value_of("executable-copy").unwrap(),
        &matches.values_of("args").unwrap().collect::<Vec<&str>>(),
      );
    }
    ("run_competing_optimizers", Some(matches)) => {
      let file = std::fs::File::open(matches.value_of("competitor-spec-file").unwrap()).unwrap();
      let competitors: Vec<CompetitorSpecification> =
        serde_json::from_reader(std::io::BufReader::new(file)).unwrap();
      competing_optimizers::run(competitors);
    }
    ("sandbox", Some(matches)) => {
      sandbox::run(PathBuf::from(matches.value_of("root-path").unwrap()));
    }
    _ => {}
  }

  //println!("ready");
  //eprintln!("Hello BtS");

  /*let mut file = std::fs::OpenOptions::new()
  .create(true)
  .append(true)
  .open(r#"C:\Users\Eli\Documents\borg_the_spire_log"#)
  .unwrap();*/

  //writeln!(file, "Hello BtS 2").unwrap();

  /*let input = std::io::stdin();
  let mut input = input.lock();
  let mut failed = false;

  loop {
    let mut buffer = String::new();
    input.read_line(&mut buffer).unwrap();
    if buffer.len() > 3 {
      let interpreted: Result<communication_mod_state::CommunicationState, _> =
        serde_json::from_str(&buffer);
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
            let mut tree = mcts::Tree::new(state);

            let start = Instant::now();
            while Instant::now() - start < Duration::from_millis(1000) {
              for _ in 0..100 {
                tree.search_step();
              }
            }
            tree.print_stuff();
          }
        }
        Err(err) => {
          eprintln!("received non-state from communication mod {:?}", err);
          if !failed {
            eprintln!("data: {:?}", buffer);
          }
          failed = true;
        }
      }
    }
  }*/
}
