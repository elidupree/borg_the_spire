#![feature(proc_macro_hygiene, decl_macro, array_map, generic_associated_types)]
#![feature(map_first_last)]

#[macro_use]
extern crate rocket;

use crate::competing_optimizers::CompetitorSpecification;
use clap::{App, AppSettings, Arg, SubCommand};
use std::path::PathBuf;
//use std::io::BufRead;

//use std::time::{Duration, Instant};

macro_rules! power_hook {
  ($runner: expr, AllMonsters, $hook: ident ( $($arguments:tt)*)) => {
    {
      let runner = &mut* $runner;
      for monster_index in 0..runner.state().monsters.len() {
        if !runner.state().monsters [monster_index].gone {
          power_hook! (runner, CreatureIndex::Monster (monster_index), $hook ($($arguments)*));
        }
      }
    }
  };
  ($runner: expr, AllCreatures, $hook: ident ( $($arguments:tt)*)) => {
    {
      let runner = &mut* $runner;
      power_hook! (runner, CreatureIndex::Player, $hook ($($arguments)*));
      power_hook! (runner, AllMonsters, $hook ($($arguments)*));
    }
  };
  ($runner: expr, $owner: expr, PowerId::$Variant: ident, $hook: ident ( $($arguments:tt)*)) => {
    {
      let runner = &mut* $runner;
            let owner = $owner;
            if let Some(index) = runner.state().get_creature (owner).powers.iter().position (| power | power.power_id == PowerId::$Variant) {
        power_hook! (runner, owner, index, $hook ($($arguments)*)) ;
      }
    }
  };
  ($runner: expr, $owner: expr, $power_index: expr, $hook: ident ( $($arguments:tt)*)) => {
    {
      let runner = &mut* $runner;
      let owner = $owner;
      let index = $power_index;
      let power_id = runner.state().get_creature (owner).powers [index].power_id;
      $crate::simulation_state::powers::PowerBehavior::$hook (&power_id, &mut $crate::simulation_state::powers::PowerHookContext {runner, owner, power_index: index}, $($arguments)*);
    }
  };
  ($runner: expr, $owner: expr, $hook: ident ( $($arguments:tt)*)) => {
    {
      let runner = &mut* $runner;
      let owner = $owner;
      let creature = runner.state().get_creature(owner);
      for index in 0..creature.powers.len() {
        power_hook! (runner, owner, index, $hook ($($arguments)*)) ;
      }
    }
  };
  ($state: expr, $owner: expr, $lval: ident = $hook: ident ( $($arguments:tt)*)) => {
    {
      let state = $state;
      let owner = $owner;
      let creature = state.get_creature(owner);
      for (index, power) in creature.powers.iter().enumerate() {
        $lval = $crate::simulation_state::powers::PowerBehavior::$hook (&power.power_id, &$crate::simulation_state::powers::PowerNumericHookContext {state, owner, power_index: index}, $($arguments)*);
      }
    }
  };
}

mod actions;
mod communication_mod_state;
mod competing_optimizers;
//mod cow;
mod ai_utils;
mod interface;
mod neural_net_ai;
//mod omniscient_search;
mod sandbox;
mod seed_system;
mod seeds_concrete;
mod simulation;
pub mod simulation_state;
mod start_and_strategy_ai;
mod watch;

fn main() {
  let matches = App::new("Borg the Spire")
    .version("0.1")
    .author("Eli Dupree <vcs@elidupree.com>")
    .subcommand(
      SubCommand::with_name("communicate").arg(Arg::with_name("root_path").required(true)),
    )
    .subcommand(
      SubCommand::with_name("watch")
        .setting(AppSettings::TrailingVarArg)
        .arg(Arg::with_name("executable_original"))
        .arg(Arg::with_name("executable_copy"))
        .arg(Arg::with_name("args").multiple(true)),
    )
    .subcommand(
      SubCommand::with_name("run_competing_optimizers").arg(Arg::with_name("competitor_spec_file")),
    )
    .subcommand(SubCommand::with_name("sandbox").arg(Arg::with_name("root_path").required(true)))
    .get_matches();

  match matches.subcommand() {
    ("communicate", Some(matches)) => {
      interface::run(PathBuf::from(matches.value_of("root_path").unwrap()));
    }
    ("watch", Some(matches)) => {
      println!("ready");
      watch::watch(
        matches.value_of("executable_original").unwrap(),
        matches.value_of("executable_copy").unwrap(),
        &matches.values_of("args").unwrap().collect::<Vec<&str>>(),
      );
    }
    ("run_competing_optimizers", Some(matches)) => {
      let file = std::fs::File::open(matches.value_of("competitor_spec_file").unwrap()).unwrap();
      let competitors: Vec<CompetitorSpecification> =
        serde_json::from_reader(std::io::BufReader::new(file)).unwrap();
      competing_optimizers::run(competitors);
    }
    ("sandbox", Some(matches)) => {
      sandbox::run(PathBuf::from(matches.value_of("root_path").unwrap()));
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
