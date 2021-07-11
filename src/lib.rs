#![feature(proc_macro_hygiene, decl_macro, array_map, generic_associated_types)]
#![feature(map_first_last)]

#[macro_use]
extern crate rocket;

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

pub mod actions;
pub mod communication_mod_state;
pub mod competing_optimizers;
//mod cow;
pub mod ai_utils;
pub mod commands {
  pub mod communicate;
  pub mod interface;
  pub mod sandbox;
  pub mod watch;
}
pub mod neural_net_ai;
//mod omniscient_search;
pub mod representative_sampling;
pub mod seed_system;
pub mod seeds_concrete;
pub mod simulation;
pub mod simulation_state;
pub mod start_and_strategy_ai;
