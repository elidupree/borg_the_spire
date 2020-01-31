#![feature(proc_macro_hygiene, decl_macro)]

#[macro_use]
extern crate rocket;

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
      power_id.$hook (&mut $crate::simulation_state::powers::PowerHookContext {runner, owner, power_index: index}, $($arguments)*);
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
        $lval = power.power_id.$hook (&$crate::simulation_state::powers::PowerNumericHookContext {state, owner, power_index: index}, $($arguments)*);
      }
    }
  };
}

mod actions;
mod communication_mod_state;
mod cow;
mod interface;
mod simulation;
mod simulation_state;
mod start_and_strategy_ai;
mod neural_net_ai;
mod benchmarks;

fn main() {
  println!("ready");
  eprintln!("Hello BtS");

  /*let mut file = std::fs::OpenOptions::new()
  .create(true)
  .append(true)
  .open(r#"C:\Users\Eli\Documents\borg_the_spire_log"#)
  .unwrap();*/

  //writeln!(file, "Hello BtS 2").unwrap();

  let arguments: Vec<String> = std::env::args().collect();
  
  if arguments[1] == "benchmarks" {
    benchmarks::run_benchmarks();
    return
  }
  
  interface::run(PathBuf::from(arguments[1].clone()));

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
