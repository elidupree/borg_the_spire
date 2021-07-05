use crate::seed_system::ChoiceLineageIdentity;
use crate::simulation_state::{CombatState, SingleCard};
use serde::{Deserialize, Serialize};

#[derive(Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Debug)]
pub struct CombatChoiceLineageIdentity {
  turn: i32,
  identity: CombatChoiceLineageIdentityWithoutTurn,
}

#[derive(Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Debug)]
pub enum CombatChoiceLineageIdentityWithoutTurn {
  DrawCard(SingleCard),
  MonsterIntent(i32),
  Other,
}

impl ChoiceLineageIdentity<CombatState> for CombatChoiceLineageIdentity {
  fn get(state: &CombatState, choice: i32) -> Self {
    unimplemented!()
  }
}
