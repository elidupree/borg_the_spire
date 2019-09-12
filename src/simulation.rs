use derivative::Derivative;
use serde::{Deserialize, Serialize};
use smallvec::{smallvec, SmallVec};
use std::collections::HashSet;
use std::ops::{Add, AddAssign, Mul};
//use rand::{Rng, SeedableRng};
use rand::seq::SliceRandom;
use rand_xoshiro::Xoshiro256StarStar;
use retain_mut::RetainMut;
type Generator = Xoshiro256StarStar;

use crate::actions::*;
pub use crate::simulation_state::cards::CardBehavior;
pub use crate::simulation_state::monsters::MonsterBehavior;
use crate::simulation_state::*;

pub trait Runner {
  fn can_apply_impl(&self, action: &impl Action) -> bool;
  fn can_apply(&self, action: &impl Action) -> bool {
    self.can_apply_impl(action) && !self.state().combat_over()
  }
  fn apply_impl(&mut self, action: &impl Action);
  fn apply(&mut self, action: &impl Action) {
    if self.state().fresh_action_queue.is_empty() && self.can_apply(action) {
      self.apply_impl(action);
    } else {
      self
        .state_mut()
        .fresh_action_queue
        .push(action.clone().into());
    }
  }
  fn state(&self) -> &CombatState;
  fn state_mut(&mut self) -> &mut CombatState;
}

#[derive(Clone, PartialEq, Serialize, Deserialize, Debug, Derivative)]
pub struct Distribution(pub SmallVec<[(f64, i32); 4]>);
impl From<i32> for Distribution {
  fn from(value: i32) -> Distribution {
    Distribution(smallvec![(1.0, value)])
  }
}
impl Mul<f64> for Distribution {
  type Output = Distribution;
  fn mul(mut self, other: f64) -> Distribution {
    for pair in &mut self.0 {
      pair.0 *= other;
    }
    self
  }
}
impl Add<Distribution> for Distribution {
  type Output = Distribution;
  fn add(mut self, other: Distribution) -> Distribution {
    self += other;
    self
  }
}
impl AddAssign<Distribution> for Distribution {
  fn add_assign(&mut self, other: Distribution) {
    for (weight, value) in other.0 {
      if let Some(existing) = self
        .0
        .iter_mut()
        .find(|(_, existing_value)| *existing_value == value)
      {
        existing.0 += weight;
      } else {
        self.0.push((weight, value));
      }
    }
  }
}
impl Distribution {
  pub fn new() -> Distribution {
    Distribution(SmallVec::new())
  }
  pub fn split(
    probability: f64,
    then_value: impl Into<Distribution>,
    else_value: impl Into<Distribution>,
  ) -> Distribution {
    (then_value.into() * probability) + (else_value.into() * (1.0 - probability))
  }
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, Debug)]
pub enum CreatureIndex {
  Player,
  Monster(usize),
}

#[derive(Clone, PartialEq, Serialize, Deserialize, Debug, Derivative)]
#[derivative(Default)]
pub enum Determinism {
  Choice,
  Random(Distribution),
  #[derivative(Default)]
  Deterministic,
}

pub trait Action: Clone + Into<DynAction> {
  fn determinism(&self, state: &CombatState) -> Determinism {
    Determinism::Deterministic
  }
  fn execute(&self, runner: &mut impl Runner) {
    panic!("an action didn't define the correct apply method for its determinism")
  }
  fn execute_random(&self, runner: &mut impl Runner, random_value: i32) {
    panic!("an action didn't define the correct apply method for its determinism")
  }
}

pub struct DefaultRunner<'a> {
  state: &'a mut CombatState,
}

impl<'a> DefaultRunner<'a> {
  pub fn new(state: &'a mut CombatState) -> Self {
    DefaultRunner { state }
  }
}

impl<'a> Runner for DefaultRunner<'a> {
  fn can_apply_impl(&self, action: &impl Action) -> bool {
    action.determinism(self.state()) != Determinism::Choice
  }
  fn apply_impl(&mut self, action: &impl Action) {
    match action.determinism(self.state()) {
      Determinism::Deterministic => action.execute(self),
      Determinism::Random(distribution) => {
        let random_value = distribution
          .0
          .choose_weighted(&mut rand::thread_rng(), |(weight, value)| *weight)
          .unwrap()
          .1;
        action.execute_random(self, random_value);
      }
      Determinism::Choice => unreachable!(),
    }
  }
  fn state(&self) -> &CombatState {
    self.state
  }
  fn state_mut(&mut self) -> &mut CombatState {
    self.state
  }
}

pub struct DeterministicRunner<'a> {
  state: &'a mut CombatState,
}

impl<'a> DeterministicRunner<'a> {
  pub fn new(state: &'a mut CombatState) -> Self {
    DeterministicRunner { state }
  }
}

impl<'a> Runner for DeterministicRunner<'a> {
  fn can_apply_impl(&self, action: &impl Action) -> bool {
    match action.determinism(self.state()) {
      Determinism::Deterministic => true,
      Determinism::Random(distribution) => distribution.0.len() == 1,
      Determinism::Choice => false,
    }
  }
  fn apply_impl(&mut self, action: &impl Action) {
    match action.determinism(self.state()) {
      Determinism::Deterministic => action.execute(self),
      Determinism::Random(distribution) => action.execute_random(self, distribution.0[0].1),
      Determinism::Choice => unreachable!(),
    }
  }
  fn state(&self) -> &CombatState {
    self.state
  }
  fn state_mut(&mut self) -> &mut CombatState {
    self.state
  }
}

pub fn run_until_unable(runner: &mut impl Runner) {
  loop {
    if runner.state().combat_over() {
      break;
    }

    while let Some(action) = runner.state_mut().fresh_action_queue.pop() {
      runner.state_mut().stale_action_stack.push(action)
    }

    if let Some(action) = runner.state_mut().stale_action_stack.pop() {
      if runner.can_apply(&action) {
        runner.apply(&action);
      } else {
        runner.state_mut().stale_action_stack.push(action);
        break;
      }
    } else {
      break;
    }
  }
}

/*#[derive(Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Choice {
  PlayCard(SingleCard, usize),
  EndTurn,
}

impl Choice {
  pub fn apply(&self, state: &mut CombatState, runner: &mut impl Runner) {
    match self {
      Choice::PlayCard(card, target) => state.play_card(runner, card, *target),
      Choice::EndTurn => state.end_turn(runner),
    }
  }
}*/

pub type Choice = DynAction;

impl Creature {
  pub fn has_power(&self, power_id: PowerId) -> bool {
    self.powers.iter().any(|power| power.power_id == power_id)
  }
  pub fn power_amount(&self, power_id: PowerId) -> i32 {
    self
      .powers
      .iter()
      .filter(|power| power.power_id == power_id)
      .map(|power| power.amount)
      .sum()
  }

  pub fn start_turn(&mut self) {
    self.block = 0;
  }

  pub fn finish_round(&mut self) {
    self.powers.retain_mut(|power| match power.power_id {
      PowerId::Vulnerable | PowerId::Weak | PowerId::Frail => {
        if power.just_applied {
          power.just_applied = false;
          true
        } else {
          power.amount -= 1;
          power.amount > 0
        }
      }
      _ => true,
    });
  }

  pub fn adjusted_damage_received(&self, mut damage: i32) -> i32 {
    if self.has_power(PowerId::Vulnerable) {
      damage = (damage * 3 + 1) / 2;
    }
    damage
  }

  pub fn adjusted_damage_dealt(&self, mut damage: i32) -> i32 {
    damage += self.power_amount(PowerId::Strength);
    if self.has_power(PowerId::Weak) {
      damage = (damage * 3) / 4;
    }
    if damage <= 0 {
      return 0;
    }
    damage
  }

  pub fn do_block(&mut self, mut amount: i32) {
    if self.has_power(PowerId::Frail) {
      amount = (amount * 3) / 4;
    }
    if amount > 0 {
      self.block += amount;
    }
  }
}

impl CombatState {
  pub fn combat_over(&self) -> bool {
    self.player.creature.hitpoints <= 0 || self.monsters.iter().all(|monster| monster.gone)
  }

  pub fn card_playable(&self, card: &SingleCard) -> bool {
    card.cost >= -1 && self.player.energy >= card.cost
  }

  pub fn legal_choices(&self) -> Vec<Choice> {
    let mut result = Vec::with_capacity(10);
    result.push(EndTurn.into());
    let mut cards = HashSet::new();
    for card in &self.hand {
      if cards.insert(card) && self.card_playable(card) {
        if card.card_info.has_target {
          for (monster_index, monster) in self.monsters.iter().enumerate() {
            if !monster.gone {
              result.push(
                PlayCard {
                  card: card.clone(),
                  target: monster_index,
                }
                .into(),
              );
            }
          }
        } else {
          result.push(
            PlayCard {
              card: card.clone(),
              target: 0,
            }
            .into(),
          );
        }
      }
    }
    result
  }

  pub fn get_creature(&self, index: CreatureIndex) -> &Creature {
    match index {
      CreatureIndex::Player => &self.player.creature,
      CreatureIndex::Monster(index) => &self.monsters[index].creature,
    }
  }

  pub fn get_creature_mut(&mut self, index: CreatureIndex) -> &mut Creature {
    match index {
      CreatureIndex::Player => &mut self.player.creature,
      CreatureIndex::Monster(index) => &mut self.monsters[index].creature,
    }
  }

  pub fn monster_intent(&self, monster_index: usize) -> i32 {
    self.monsters[monster_index].intent()
  }
}

impl Monster {
  pub fn intent(&self) -> i32 {
    *self.move_history.last().unwrap()
  }
  pub fn push_intent(&mut self, intent: i32) {
    if self.move_history.len() == 3 {
      self.move_history.remove(0);
    }
    self.move_history.push(intent);
  }
}
