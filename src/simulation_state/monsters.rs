#![allow (unused_variables)]

use rand::Rng;
use serde::{Deserialize, Serialize};
use std::convert::From;

use crate::simulation::*;
use crate::simulation_state::*;

pub trait MonsterBehavior {
  fn choose_next_intent(self, monster: &mut Monster, runner: &mut impl Runner);
  fn enact_intent(self, state: &mut CombatState, runner: &mut impl Runner, monster_index: usize);
}

macro_rules! monsters {
  ($([$id: expr, $Variant: ident],)*) => {
    #[derive(Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Debug)]
    pub enum MonsterId {
      $($Variant,)*
    }

    $(#[derive(Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Debug)]
    pub struct $Variant;)*

    impl From<& str> for MonsterId {
      fn from (source: & str)->MonsterId {
        match source {
          $($id => MonsterId::$Variant,)*
          _ => MonsterId::Cultist,
        }
      }
    }

    impl MonsterBehavior for MonsterId {
      fn choose_next_intent (self, monster: &mut Monster, runner: &mut impl Runner) {
        match self {
        $(MonsterId::$Variant => $Variant.choose_next_intent (monster, runner),)*
        }
      }
      fn enact_intent (self, state: &mut CombatState, runner: &mut impl Runner, monster_index: usize) {
        match self {
        $(MonsterId::$Variant => $Variant.enact_intent (state, runner, monster_index),)*
                }
      }
    }
  }
}

monsters! {
  ["FuzzyLouseNormal", RedLouse],
  ["FuzzyLouseDefensive", GreenLouse],
  ["Cultist", Cultist],
  ["JawWorm", JawWorm],
  ["AcidSlime_S", AcidSlimeS],
  ["AcidSlime_M", AcidSlimeM],
  ["SpikeSlime_S", SpikeSlimeS],
  ["SpikeSlime_M", SpikeSlimeM],
}

impl MonsterBehavior for Cultist {
  fn choose_next_intent(self, monster: &mut Monster, runner: &mut impl Runner) {
    if monster.move_history.is_empty() {
      monster.push_intent (3);
    } else {
      monster.push_intent(1);
    }
  }
  fn enact_intent(self, state: &mut CombatState, runner: &mut impl Runner, monster_index: usize) {
    let intent = state.monster_intent(monster_index);
    match intent {
      3 => {
        let monster = &mut state.monsters[monster_index];
        monster.creature.apply_power_amount(
          PowerId::Ritual,
          if monster.ascension >= 17 { 4 } else { 3 },
          true,
        );
      }
      1 => {
        state.monster_attacks_player(runner, monster_index, 6, 1);
      }
      _ => eprintln!(" Unknown intent for Cultist: {:?} ", intent),
    }
  }
}

impl MonsterBehavior for RedLouse {
  fn choose_next_intent(self, monster: &mut Monster, runner: &mut impl Runner) {
    monster.push_intent(runner.gen(|generator| {
      let num = generator.gen_range(0, 100);
      if monster.ascension >= 17 {
        if num < 25 {
          if monster.last_move(4) {
            3
          } else {
            4
          }
        } else {
          if monster.last_two_moves(3) {
            4
          } else {
            3
          }
        }
      } else {
        if num < 25 {
          if monster.last_two_moves(4) {
            3
          } else {
            4
          }
        } else {
          if monster.last_two_moves(3) {
            4
          } else {
            3
          }
        }
      }
    }));
    if monster.intent() == 3 && monster.innate_damage_amount.is_none() {
      monster.innate_damage_amount = Some(
        runner
          .gen(|generator| generator.gen_range(5, 8) + if monster.ascension >= 2 { 1 } else { 0 }),
      );
    }
  }
  fn enact_intent(self, state: &mut CombatState, runner: &mut impl Runner, monster_index: usize) {
    let intent = state.monster_intent(monster_index);

    match intent {
      4 => {
        let monster = &mut state.monsters[monster_index];
        monster.creature.apply_power_amount(
          PowerId::Strength,
          if monster.ascension >= 17 { 4 } else { 3 },
          true,
        );
      }
      3 => {
        state.monster_attacks_player(
          runner,
          monster_index,
          state.monsters[monster_index].innate_damage_amount.unwrap(),
          1,
        );
      }
      _ => eprintln!(" Unknown intent for RedLouse: {:?} ", intent),
    }
  }
}

impl MonsterBehavior for GreenLouse {
  fn choose_next_intent(self, monster: &mut Monster, runner: &mut impl Runner) {
    RedLouse.choose_next_intent(monster, runner)
  }
  fn enact_intent(self, state: &mut CombatState, runner: &mut impl Runner, monster_index: usize) {
    let intent = state.monster_intent(monster_index);

    match intent {
      4 => {
        state
          .player
          .creature
          .apply_power_amount(PowerId::Weak, 2, true);
      }
      3 => {
        state.monster_attacks_player(
          runner,
          monster_index,
          state.monsters[monster_index].innate_damage_amount.unwrap(),
          1,
        );
      }
      _ => eprintln!(" Unknown intent for GreenLouse: {:?} ", intent),
    }
  }
}

impl MonsterBehavior for JawWorm {
  fn choose_next_intent(self, monster: &mut Monster, runner: &mut impl Runner) {
    if monster.move_history.is_empty() {
      monster.push_intent(1);

      return;
    }
    monster.push_intent(runner.gen(|generator| {
      let num = generator.gen_range(0, 100);
      if num < 25 {
        if monster.last_move(1) {
          if generator.gen_bool(0.5625) {
            2
          } else {
            3
          }
        } else {
          1
        }
      } else if num < 55 {
        if monster.last_two_moves(3) {
          if generator.gen_bool(0.357) {
            1
          } else {
            2
          }
        } else {
          3
        }
      } else {
        if monster.last_move(2) {
          if generator.gen_bool(0.416) {
            1
          } else {
            3
          }
        } else {
          2
        }
      }
    }));
  }
  fn enact_intent(self, state: &mut CombatState, runner: &mut impl Runner, monster_index: usize) {
    let intent = state.monster_intent(monster_index);

    match intent {
      1 => {
        let ascension = state.monsters[monster_index].ascension;
        state.monster_attacks_player(
          runner,
          monster_index,
          if ascension >= 2 { 12 } else { 11 },
          1,
        );
      }
      2 => {
        let monster = &mut state.monsters[monster_index];
        monster.creature.apply_power_amount(
          PowerId::Strength,
          if monster.ascension >= 17 {
            5
          } else if monster.ascension >= 2 {
            4
          } else {
            3
          },
          false,
        );
        monster.creature.block += if monster.ascension >= 17 { 9 } else { 6 };
      }
      3 => {
        state.monster_attacks_player(runner, monster_index, 7, 1);
        let monster = &mut state.monsters[monster_index];
        monster.creature.block += 5;
      }
      _ => eprintln!(" Unknown intent for JawWorm: {:?} ", intent),
    }
  }
}

impl MonsterBehavior for AcidSlimeS {
  fn choose_next_intent(self, monster: &mut Monster, runner: &mut impl Runner) {}
  fn enact_intent(self, state: &mut CombatState, runner: &mut impl Runner, monster_index: usize) {
    let intent = state.monster_intent(monster_index);
  }
}

impl MonsterBehavior for AcidSlimeM {
  fn choose_next_intent(self, monster: &mut Monster, runner: &mut impl Runner) {}
  fn enact_intent(self, state: &mut CombatState, runner: &mut impl Runner, monster_index: usize) {
    let intent = state.monster_intent(monster_index);
  }
}

impl MonsterBehavior for SpikeSlimeS {
  fn choose_next_intent(self, monster: &mut Monster, runner: &mut impl Runner) {}
  fn enact_intent(self, state: &mut CombatState, runner: &mut impl Runner, monster_index: usize) {
    let intent = state.monster_intent(monster_index);
  }
}

impl MonsterBehavior for SpikeSlimeM {
  fn choose_next_intent(self, monster: &mut Monster, runner: &mut impl Runner) {}
  fn enact_intent(self, state: &mut CombatState, runner: &mut impl Runner, monster_index: usize) {
    let intent = state.monster_intent(monster_index);
  }
}
