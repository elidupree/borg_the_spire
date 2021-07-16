use crate::actions::{
  AttackDamageRandomEnemyAction, ChooseMonsterIntent, DynAction, GainBlockRandomMonsterAction,
  InitializeMonsterInnateDamageAmount,
};
use crate::seed_system::{
  ChoiceLineageIdentity, ChoiceLineages, ChoiceLineagesKind, GameState, MaybeSeedView, NeverSeed,
  NoRandomness,
};
use crate::simulation_state::monsters::MAX_INTENTS;
use crate::simulation_state::{CardId, CombatState, MAX_MONSTERS};
use enum_map::EnumMap;
use smallvec::SmallVec;
use std::fmt::Debug;

impl GameState for CombatState {
  type RandomForkType = DynAction;
  type RandomChoice = i32;
}
#[derive(Clone, Debug, Default)]
struct TurnMap<T> {
  values: Vec<T>,
}

impl<T: Default> TurnMap<T> {
  fn get_mut(&mut self, turn: u8) -> &mut T {
    let turn = turn as usize - 1;
    if turn >= self.values.len() {
      self.values.resize_with(turn + 1, Default::default);
    }
    &mut self.values[turn]
  }
}

#[derive(Copy, Clone, Hash, Debug)]
pub enum CombatChoiceLineageIdentity {
  DrawCard {
    card: CardId,
    turn: u8,
    reshuffles: u8,
  },
  ChooseMonsterIntent {
    turn: u8,
    monster_index: u8,
    intent: u8,
  },
  AttackRandomEnemy {
    target: u8,
  },
  InitializeMonsterInnateDamageAmount {
    monster_index: u8,
  },
  GainBlockRandomMonster {
    turn: u8,
    target: u8,
  },
  Uncategorized,
}
#[derive(Clone, Debug, Default)]
pub struct CombatChoiceLineages<T> {
  draw_card: EnumMap<CardId, TurnMap<SmallVec<[(u8, T); 2]>>>,
  choose_monster_intent: [TurnMap<[T; MAX_INTENTS]>; MAX_MONSTERS],
  attack_random_enemy: [T; MAX_MONSTERS],
  initialize_monster_innate_damage_amount: [T; MAX_MONSTERS],
  gain_block_random_monster: [TurnMap<T>; MAX_MONSTERS],
  uncategorized: T,
}
pub struct CombatChoiceLineagesKind;

impl ChoiceLineageIdentity<CombatState> for CombatChoiceLineageIdentity {
  #[inline(always)]
  fn lineage_identity(state: &CombatState, action: &DynAction, &choice: &i32) -> Self {
    match action {
      DynAction::DrawCardRandom(_) => CombatChoiceLineageIdentity::DrawCard {
        card: state.draw_pile[choice as usize].card_info.id,
        turn: state.turn_number as u8,
        reshuffles: state.num_reshuffles as u8,
      },
      &DynAction::ChooseMonsterIntent(ChooseMonsterIntent(monster_index)) => {
        CombatChoiceLineageIdentity::ChooseMonsterIntent {
          turn: state.turn_number as u8,
          monster_index: monster_index as u8,
          intent: choice as u8,
        }
      }
      &DynAction::AttackDamageRandomEnemyAction(AttackDamageRandomEnemyAction { .. }) => {
        CombatChoiceLineageIdentity::AttackRandomEnemy {
          target: choice as u8,
        }
      }
      &DynAction::InitializeMonsterInnateDamageAmount(InitializeMonsterInnateDamageAmount {
        monster_index,
        ..
      }) => CombatChoiceLineageIdentity::InitializeMonsterInnateDamageAmount {
        monster_index: monster_index as u8,
      },
      DynAction::GainBlockRandomMonsterAction(GainBlockRandomMonsterAction { .. }) => {
        CombatChoiceLineageIdentity::GainBlockRandomMonster {
          turn: state.turn_number as u8,
          target: choice as u8,
        }
      }
      _ => CombatChoiceLineageIdentity::Uncategorized,
    }
  }
}

impl<T: Clone + Debug + Default> ChoiceLineages for CombatChoiceLineages<T> {
  type LineageIdentity = CombatChoiceLineageIdentity;
  type Lineage = T;
  #[inline(always)]
  fn get_mut(&mut self, identity: CombatChoiceLineageIdentity) -> &mut T {
    match identity {
      CombatChoiceLineageIdentity::DrawCard {
        card,
        turn,
        reshuffles,
      } => {
        let turn = self.draw_card[card].get_mut(turn);
        if let Some(i) = turn.iter().position(|&(r, _)| r == reshuffles) {
          &mut turn[i].1
        } else {
          turn.push((reshuffles, Default::default()));
          &mut turn.last_mut().unwrap().1
        }
      }
      CombatChoiceLineageIdentity::ChooseMonsterIntent {
        turn,
        monster_index,
        intent,
      } => &mut self.choose_monster_intent[monster_index as usize].get_mut(turn)[intent as usize],
      CombatChoiceLineageIdentity::AttackRandomEnemy { target } => {
        &mut self.attack_random_enemy[target as usize]
      }
      CombatChoiceLineageIdentity::InitializeMonsterInnateDamageAmount { monster_index } => {
        &mut self.initialize_monster_innate_damage_amount[monster_index as usize]
      }
      CombatChoiceLineageIdentity::GainBlockRandomMonster { turn, target } => {
        self.gain_block_random_monster[target as usize].get_mut(turn)
      }
      CombatChoiceLineageIdentity::Uncategorized => &mut self.uncategorized,
    }
  }
}

impl ChoiceLineagesKind for CombatChoiceLineagesKind {
  type LineageIdentity = CombatChoiceLineageIdentity;
  type Lineages<T: Clone + Debug + Default> = CombatChoiceLineages<T>;
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
