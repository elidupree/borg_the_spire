use derivative::Derivative;
use serde::{Deserialize, Serialize};
//use rand::{Rng, SeedableRng};

use crate::actions::*;
use crate::seed_system::{choose_choice, Distribution, MaybeSeedView};
pub use crate::simulation_state::cards::CardBehavior;
pub use crate::simulation_state::monsters::MonsterBehavior;
use crate::simulation_state::*;
use smallvec::alloc::fmt::Formatter;
use std::fmt;
use std::fmt::Display;

/*
pub enum CardChoiceType {
  ExhaustCard,
  HandTopdeck,
  DiscardTopdeck,
  TutorSkill,
  TutorAttack,
}
*/

pub const HARD_ACTION_LIMIT: i32 = 10000;

pub type MonsterIndex = usize;

#[derive(Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, Debug)]
pub enum CreatureIndex {
  Player,
  Monster(MonsterIndex),
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, Debug)]
pub enum DamageType {
  Normal,
  Thorns,
  HitpointLoss,
}

#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, Debug)]
pub struct DamageInfo {
  pub damage_type: DamageType,
  pub owner: CreatureIndex,
  pub base: i32,
  pub output: i32,
}

impl DamageInfo {
  pub fn new(source: CreatureIndex, base: i32, damage_type: DamageType) -> DamageInfo {
    DamageInfo {
      owner: source,
      base,
      damage_type,
      output: base,
    }
  }
  pub fn apply_powers(&mut self, state: &CombatState, owner: CreatureIndex, target: CreatureIndex) {
    self.output = self.base;
    let mut damage = self.output as f64;
    power_hook!(
      state,
      owner,
      damage = at_damage_give(damage, self.damage_type)
    );
    power_hook!(
      state,
      target,
      damage = at_damage_receive(damage, self.damage_type)
    );
    power_hook!(
      state,
      target,
      damage = at_damage_final_receive(damage, self.damage_type)
    );
    self.output = damage as i32;
    if self.output < 0 {
      self.output = 0
    }
  }
}

#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, Debug)]
pub enum PowerType {
  Buff,
  Debuff,
  Relic,
}

#[derive(Clone, PartialEq, Serialize, Deserialize, Debug, Derivative)]
#[derivative(Default)]
pub enum Determinism {
  Choice,
  Random(Distribution<i32>),
  #[derivative(Default)]
  Deterministic,
}

pub trait Action: Clone + Into<DynAction> {
  #[allow(unused)]
  fn determinism(&self, state: &CombatState) -> Determinism {
    Determinism::Deterministic
  }
  #[allow(unused)]
  fn execute(&self, runner: &mut impl Runner) {
    panic!("an action didn't define the correct apply method for its determinism")
  }
  #[allow(unused)]
  fn execute_random(&self, runner: &mut impl Runner, random_value: i32) {
    panic!("an action didn't define the correct apply method for its determinism")
  }
}

pub trait Runner {
  fn state(&self) -> &CombatState;
  fn state_mut(&mut self) -> &mut CombatState;

  fn action_now(&mut self, action: &impl Action);
  fn action_top(&mut self, action: impl Action);
  fn action_bottom(&mut self, action: impl Action);

  fn run_until_unable(&mut self);
  fn apply_choice(&mut self, choice: &Choice);
}

pub struct StandardRunner<'a, Seed> {
  state: &'a mut CombatState,
  seed: Seed,
  hooks: Option<&'a mut dyn StandardRunnerHooks>,
}
#[allow(unused)]
pub trait StandardRunnerHooks {
  fn on_choice(&mut self, state: &CombatState, choice: &Choice) {}
  fn on_action(&mut self, state: &CombatState, action: &DynAction) {}
}

impl<'a, Seed: MaybeSeedView<CombatState>> StandardRunner<'a, Seed> {
  pub fn new(state: &'a mut CombatState, seed: Seed) -> Self {
    StandardRunner {
      state,
      seed,
      hooks: None,
    }
  }
  pub fn with_hooks(mut self, hooks: &'a mut dyn StandardRunnerHooks) -> Self {
    self.hooks = Some(hooks);
    self
  }

  fn can_apply(&self, action: &impl Action) -> bool {
    self.can_apply_impl(action) && !self.state().combat_over()
  }
  fn can_apply_impl(&self, action: &impl Action) -> bool {
    match action.determinism(self.state()) {
      Determinism::Deterministic => true,
      Determinism::Random(distribution) => self.seed.is_seed() || distribution.0.len() == 1,
      Determinism::Choice => false,
    }
  }
  fn apply_impl(&mut self, action: &impl Action) {
    self.state.num_actions += 1;
    if let Some(hooks) = &mut self.hooks {
      hooks.on_action(&self.state, &action.clone().into());
    }
    // if self.debug {
    //   writeln!(
    //     self.log,
    //     "Applying {:?} to state {:?}",
    //     action.clone().into(),
    //     self.state
    //   )
    //   .unwrap();
    // }
    match action.determinism(self.state()) {
      Determinism::Deterministic => action.execute(self),
      Determinism::Random(distribution) => {
        let random_value = match self.seed.as_seed() {
          Some(seed) => choose_choice(&*self.state, &action.clone().into(), &distribution, seed),
          None => {
            assert_eq!(distribution.0.len(), 1);
            distribution.0.first().unwrap().1
          }
        };
        action.execute_random(self, random_value);
      }
      Determinism::Choice => unreachable!(),
    }
    // if self.debug {
    //   writeln!(
    //     self.log,
    //     "Done applying {:?}; state is now {:?}",
    //     action.clone().into(),
    //     self.state
    //   )
    //   .unwrap();
    // }
  }
  // pub fn debug_log(&self) -> &str {
  //   &self.log
  // }
}
impl<'a, Seed: MaybeSeedView<CombatState>> Runner for StandardRunner<'a, Seed> {
  fn state(&self) -> &CombatState {
    self.state
  }
  fn state_mut(&mut self) -> &mut CombatState {
    self.state
  }

  fn action_now(&mut self, action: &impl Action) {
    if self.state().fresh_subaction_queue.is_empty() && self.can_apply(action) {
      self.apply_impl(action);
    } else {
      self
        .state_mut()
        .fresh_subaction_queue
        .push(action.clone().into());
    }
  }
  fn action_top(&mut self, action: impl Action) {
    self.state_mut().actions.push_front(action.into());
  }
  fn action_bottom(&mut self, action: impl Action) {
    self.state_mut().actions.push_back(action.into());
  }

  fn run_until_unable(&mut self) {
    loop {
      if self.state().combat_over() {
        break;
      }

      while let Some(action) = self.state_mut().fresh_subaction_queue.pop() {
        self.state_mut().stale_subaction_stack.push(action)
      }

      if let Some(action) = self.state_mut().stale_subaction_stack.pop() {
        if self.can_apply(&action) {
          self.action_now(&action);
        } else {
          self.state_mut().stale_subaction_stack.push(action);
          break;
        }
      } else {
        if let Some(action) = self.state_mut().actions.pop_front() {
          self.action_now(&action);
        } else {
          break;
        }
      }
    }
  }
  fn apply_choice(&mut self, choice: &Choice) {
    assert!(self.state().fresh_subaction_queue.is_empty());
    assert!(self.state().stale_subaction_stack.is_empty());
    assert!(self.state().actions.is_empty());
    if let Some(hooks) = &mut self.hooks {
      hooks.on_choice(&self.state, choice);
    }
    self.state.num_choices += 1;
    self.apply_impl(choice);
    self.run_until_unable();
  }
}

/*#[derive(Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Choice {
  PlayCard(SingleCard, usize),
  EndTurn,
}

impl Choice {
  pub fn apply(&self, state: &mut CombatState, runner: &mut Runner) {
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
}

impl CombatState {
  pub fn combat_over(&self) -> bool {
    self.player.creature.hitpoints <= 0
      || self.monsters.iter().all(|monster| monster.gone)
      || self.num_actions >= HARD_ACTION_LIMIT
  }
  pub fn choice_next(&self) -> bool {
    (!self.combat_over()) && self.stale_subaction_stack.is_empty()
  }

  pub fn card_playable(&self, card: &SingleCard) -> bool {
    assert!(X_COST == -1);
    assert!(UNPLAYABLE == -2);
    card.cost >= -1
      && self.player.energy >= card.cost
      && card.card_info.id.playable(self)
      && !(card.card_info.card_type == CardType::Attack
        && self.player.creature.has_power(PowerId::Entangled))
  }

  pub fn legal_choices(&self) -> Vec<Choice> {
    let mut result = Vec::with_capacity(10);
    result.push(EndTurn.into());
    for (index, card) in self.hand.iter().enumerate() {
      if self.hand[..index]
        .iter()
        .all(|earlier_card| earlier_card != card)
        && self.card_playable(card)
      {
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

  pub fn monster_intent(&self, monster_index: usize) -> IntentId {
    self.monsters[monster_index].intent()
  }
}

impl Monster {
  pub fn intent(&self) -> IntentId {
    *self.move_history.last().unwrap()
  }
  pub fn push_intent(&mut self, intent: IntentId) {
    /*if self.move_history.len() == 3 {
      self.move_history.remove(0);
    }*/
    self.move_history.push(intent);
  }
}

pub trait ConsiderAction {
  fn consider(&mut self, action: impl Action);
}

impl Display for Choice {
  fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
    match self {
      Choice::PlayCard(PlayCard { card, target }) => {
        if card.card_info.has_target {
          write!(f, "{} {}", card, target)
        } else {
          write!(f, "{}", card)
        }
      }
      Choice::EndTurn(_) => {
        write!(f, "EndTurn")
      }
      _ => {
        write!(f, "<invalid Choice: {:?}>", self)
      }
    }
  }
}

pub struct DisplayChoices<'a>(pub &'a [Choice]);

impl<'a> Display for DisplayChoices<'a> {
  fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
    let list = self
      .0
      .iter()
      .map(ToString::to_string)
      .collect::<Vec<_>>()
      .join(", ");
    write!(f, "[{}]", list)
  }
}
