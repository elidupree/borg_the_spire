use crate::seed_system::{ChoiceLineageIdentity, MaybeSeedView, NeverSeed, NoRandomness};
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

// This would prefer to live in the seed_system module, but it can't be implemented generically due to details of the orphan rule
impl MaybeSeedView<CombatState> for NoRandomness {
  type SelfAsSeed = NeverSeed;
  fn is_seed(&self) -> bool {
    false
  }
  fn as_seed(&mut self) -> Option<&mut Self::SelfAsSeed> {
    None
  }
}
