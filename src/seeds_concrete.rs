use crate::actions::{
  AttackDamageRandomEnemyAction, ChooseMonsterIntent, DynAction, GainBlockRandomMonsterAction,
  InitializeMonsterInnateDamageAmount,
};
use crate::seed_system::{
  ChoiceLineages, ContainerKind, GameState, MaybeSeedView, NeverSeed, NoRandomness,
};
use crate::simulation::MonsterIndex;
use crate::simulation_state::{CardId, CombatState, MAX_MONSTERS};
use enum_map::EnumMap;
use smallvec::SmallVec;
use std::collections::HashMap;
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
  fn get_mut(&mut self, turn: i32) -> &mut T {
    let turn = turn as usize - 1;
    if turn >= self.values.len() {
      self.values.resize_with(turn + 1, Default::default);
    }
    &mut self.values[turn]
  }
}

#[derive(Clone, Debug, Default)]
pub struct CombatChoiceLineages<T> {
  draw_card: EnumMap<CardId, TurnMap<SmallVec<[(i32, T); 2]>>>,
  choose_monster_intent: [TurnMap<HashMap<i32, T>>; MAX_MONSTERS],
  attack_random_enemy: [HashMap<i32, T>; MAX_MONSTERS],
  initialize_monster_innate_damage_amount: [T; MAX_MONSTERS],
  gain_block_random_monster: [TurnMap<T>; MAX_MONSTERS],
  uncategorized: T,
}
pub struct CombatChoiceLineagesKind;

impl<T: Default> ChoiceLineages<CombatState> for CombatChoiceLineages<T> {
  type Lineage = T;
  fn get_mut(&mut self, state: &CombatState, action: &DynAction, &choice: &i32) -> &mut T {
    match action {
      DynAction::DrawCardRandom(_) => {
        let card = state.draw_pile[choice as usize].clone();
        let turn = self.draw_card[card.card_info.id].get_mut(state.turn_number);
        if let Some(i) = turn.iter().position(|&(r, _)| r == state.num_reshuffles) {
          &mut turn[i].1
        } else {
          turn.push((state.num_reshuffles, Default::default()));
          &mut turn.last_mut().unwrap().1
        }
      }
      &DynAction::ChooseMonsterIntent(ChooseMonsterIntent(monster_index)) => {
        let intent = choice;
        self.choose_monster_intent[monster_index]
          .get_mut(state.turn_number)
          .entry(intent)
          .or_insert_with(Default::default)
      }
      &DynAction::AttackDamageRandomEnemyAction(AttackDamageRandomEnemyAction { damage }) => {
        let target = choice as MonsterIndex;
        self.attack_random_enemy[target]
          .entry(damage)
          .or_insert_with(Default::default)
      }
      &DynAction::InitializeMonsterInnateDamageAmount(InitializeMonsterInnateDamageAmount {
        monster_index,
        ..
      }) => self
        .initialize_monster_innate_damage_amount
        .get_mut(monster_index)
        .unwrap(),
      DynAction::GainBlockRandomMonsterAction(GainBlockRandomMonsterAction { .. }) => {
        let target = choice as MonsterIndex;
        self.gain_block_random_monster[target].get_mut(state.turn_number)
      }
      _ => &mut self.uncategorized,
    }
  }
}

impl ContainerKind for CombatChoiceLineagesKind {
  type Container<T: Clone + Debug + Default> = CombatChoiceLineages<T>;
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
