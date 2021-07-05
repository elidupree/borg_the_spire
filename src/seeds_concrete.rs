use crate::actions::{
  AttackDamageRandomEnemyAction, ChooseMonsterIntent, DynAction, GainBlockRandomMonsterAction,
  InitializeMonsterInnateDamageAmount,
};
use crate::seed_system::{
  ChoiceLineageIdentity, GameState, MaybeSeedView, NeverSeed, NoRandomness,
};
use crate::simulation::MonsterIndex;
use crate::simulation_state::{CombatState, SingleCard};
use serde::{Deserialize, Serialize};

impl GameState for CombatState {
  type RandomForkType = DynAction;
  type RandomChoice = i32;
}

#[derive(Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Debug)]
pub enum CombatChoiceLineageIdentity {
  DrawCard {
    turn: i32,
    card: SingleCard,
  },
  ChooseMonsterIntent {
    turn: i32,
    monster_index: MonsterIndex,
    intent: i32,
  },
  AttackRandomEnemy {
    target: MonsterIndex,
    damage: i32,
  },
  InitializeMonsterInnateDamageAmount {
    monster_index: MonsterIndex,
  },
  GainBlockRandomMonster {
    turn: i32,
    target: MonsterIndex,
  },
  Uncategorized {
    value: i32,
  },
}

impl ChoiceLineageIdentity<CombatState> for CombatChoiceLineageIdentity {
  fn get(state: &CombatState, action: &DynAction, &choice: &i32) -> Self {
    match action {
      DynAction::DrawCardRandom(_) => CombatChoiceLineageIdentity::DrawCard {
        turn: state.turn_number,
        card: state.draw_pile[choice as usize].clone(),
      },
      &DynAction::ChooseMonsterIntent(ChooseMonsterIntent(monster_index)) => {
        CombatChoiceLineageIdentity::ChooseMonsterIntent {
          turn: state.turn_number,
          monster_index,
          intent: choice,
        }
      }
      &DynAction::AttackDamageRandomEnemyAction(AttackDamageRandomEnemyAction { damage }) => {
        CombatChoiceLineageIdentity::AttackRandomEnemy {
          target: choice as MonsterIndex,
          damage,
        }
      }
      &DynAction::InitializeMonsterInnateDamageAmount(InitializeMonsterInnateDamageAmount {
        monster_index,
        ..
      }) => CombatChoiceLineageIdentity::InitializeMonsterInnateDamageAmount { monster_index },
      DynAction::GainBlockRandomMonsterAction(GainBlockRandomMonsterAction { .. }) => {
        CombatChoiceLineageIdentity::GainBlockRandomMonster {
          turn: state.turn_number,
          target: choice as MonsterIndex,
        }
      }
      _ => CombatChoiceLineageIdentity::Uncategorized { value: choice },
    }
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
