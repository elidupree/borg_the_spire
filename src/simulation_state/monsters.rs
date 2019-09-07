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

    impl From<& str> for MonsterId {
      fn from (source: & str)->MonsterId {
        match source {
          $($id => MonsterId::$Variant,)*
          _ => MonsterId::Cultist,
        }
      }
    }

    impl MonsterBehavior for MonsterId {
      fn choose_next_intent (self, monster: &mut Monster, runner: &mut impl Runner) {}
      fn enact_intent (self, state: &mut CombatState, runner: &mut impl Runner, monster_index: usize) {}
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
