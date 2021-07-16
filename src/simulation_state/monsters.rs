#![allow(unused_variables)]

use serde::{Deserialize, Serialize};
use std::convert::From;

use crate::seed_system::Distribution;
use crate::simulation::*;
use crate::simulation_state::*;
use enum_map::Enum;

pub struct IntentChoiceContext<'a> {
  pub state: &'a CombatState,
  pub monster_index: usize,
  pub monster: &'a Monster,
  pub ascension: i32,
  num_distribution: Vec<(i32, Distribution<IntentId>)>,
}

impl<'a> IntentChoiceContext<'a> {
  pub fn if_num_lt(&mut self, threshold: i32, value: impl Into<Distribution<IntentId>>) {
    if threshold
      > self
        .num_distribution
        .last()
        .map_or(0, |(excluded, _)| *excluded)
    {
      self.num_distribution.push((threshold, value.into()));
    }
  }
  pub fn if_num_leq(&mut self, threshold: i32, value: impl Into<Distribution<IntentId>>) {
    self.if_num_lt(threshold + 1, value);
  }
  pub fn if_num_geq(&mut self, threshold: i32, value: impl Into<Distribution<IntentId>>) {
    // hack, assume that no function checks both greater and less
    self.if_num_lt(100 - threshold, value);
  }
  pub fn else_num(&mut self, value: impl Into<Distribution<IntentId>>) {
    self.if_num_lt(100, value);
  }
  pub fn always(&mut self, value: impl Into<Distribution<IntentId>>) {
    self.if_num_lt(100, value);
  }
  pub fn first_move(&self) -> bool {
    self.monster.move_history.is_empty()
  }
  pub fn last_intent<T: Intent>(&self) -> Option<T> {
    self.monster.move_history.last().map(|&id| T::from_id(id))
  }
  pub fn state(&self) -> &CombatState {
    &self.state
  }
  pub fn monster_index(&self) -> usize {
    self.monster_index
  }

  pub fn creature_index(&self) -> CreatureIndex {
    CreatureIndex::Monster(self.monster_index())
  }
  pub fn monster(&self) -> &Monster {
    &self.state().monsters[self.monster_index()]
  }

  pub fn did_repeats(&self, repeats: Repeats, intent: impl Into<IntentId>) -> bool {
    let intent = intent.into();
    self.monster.move_history.len() >= repeats.0
      && self.monster.move_history[self.monster.move_history.len() - repeats.0..]
        .iter()
        .all(|historical| *historical == intent)
  }
  pub fn with_max_repeats(
    &self,
    max_repeats: Repeats,
    intent: impl Into<IntentId>,
    alternative: impl Into<Distribution<IntentId>>,
  ) -> Distribution<IntentId> {
    let intent = intent.into();
    if self.did_repeats(max_repeats, intent) {
      alternative.into()
    } else {
      intent.into()
    }
  }

  fn ascension(&self) -> i32 {
    self.ascension
  }
  fn with_ascension<T>(&self, threshold: Ascension, upgraded: T, normal: T) -> T {
    if self.ascension() >= threshold.0 {
      upgraded
    } else {
      normal
    }
  }

  pub fn final_distribution(self) -> Option<Distribution<i32>> {
    if self.num_distribution.is_empty() {
      return None;
    }
    let mut start = 0;
    let mut result = Distribution::new();
    for (excluded, distribution) in self.num_distribution {
      result += distribution * ((excluded - start) as f64 / 100.0);
      start = excluded;
    }
    Some(result)
  }
}

fn split(
  probability: f64,
  then_value: impl Into<IntentId>,
  else_value: impl Into<IntentId>,
) -> Distribution<IntentId> {
  Distribution::split(probability, then_value.into(), else_value.into())
}

pub fn intent_choice_distribution(
  state: &CombatState,
  monster_index: usize,
) -> Option<Distribution<i32>> {
  let state = state;
  let monster = &state.monsters[monster_index];

  let monster_id = monster.monster_id;
  let mut context = IntentChoiceContext {
    state,
    monster,
    monster_index,
    ascension: monster.ascension,
    num_distribution: Vec::new(),
  };
  monster_id.make_intent_distribution(&mut context);
  context.final_distribution()
}

pub struct DoIntentContext<'a, R: Runner> {
  pub runner: &'a mut R,
  pub monster_index: usize,
}

pub struct ConsiderIntentContext<'a, F> {
  pub state: &'a CombatState,
  pub monster_index: usize,
  pub consider_action: &'a mut F,
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Debug)]
pub struct Ascension(pub i32);
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Debug)]
pub struct Repeats(pub usize);

pub trait IntentEffectsContext {
  fn action(&mut self, action: impl Action);
  fn state(&self) -> &CombatState;
  fn monster_index(&self) -> usize;

  fn creature_index(&self) -> CreatureIndex {
    CreatureIndex::Monster(self.monster_index())
  }
  fn monster(&self) -> &Monster {
    &self.state().monsters[self.monster_index()]
  }
  fn intent<T: Intent>(&self) -> T {
    T::from_id(self.monster().intent())
  }
  fn set_intent(&mut self, intent: impl Intent);
  fn ascension(&self) -> i32 {
    self.monster().ascension
  }
  fn with_ascension<T>(&self, threshold: Ascension, upgraded: T, normal: T) -> T {
    if self.ascension() >= threshold.0 {
      upgraded
    } else {
      normal
    }
  }
  fn with_ascensions<T>(
    &self,
    highest_threshold: Ascension,
    highest: T,
    threshold: Ascension,
    upgraded: T,
    normal: T,
  ) -> T {
    if self.ascension() >= highest_threshold.0 {
      highest
    } else {
      self.with_ascension(threshold, upgraded, normal)
    }
  }

  fn attack(&mut self, base_damage: i32) {
    // hack: this is actually NOT where powers are applied to card/monster damage in the actual code
    let mut info = DamageInfo::new(self.creature_index(), base_damage, DamageType::Normal);
    info.apply_powers(self.state(), self.creature_index(), CreatureIndex::Player);
    self.action(DamageAction {
      info,
      target: CreatureIndex::Player,
    });
  }
  fn power_self(&mut self, power_id: PowerId, amount: i32) {
    self.action(ApplyPowerAction {
      source: self.creature_index(),
      target: self.creature_index(),
      power_id,
      amount,
    });
  }
  fn power_player(&mut self, power_id: PowerId, amount: i32) {
    self.action(ApplyPowerAction {
      source: self.creature_index(),
      target: CreatureIndex::Player,
      power_id,
      amount,
    });
  }
  fn block(&mut self, amount: i32) {
    self.action(GainBlockAction {
      creature_index: self.creature_index(),
      amount,
    });
  }
  fn discard_status(&mut self, card_id: CardId, amount: i32) {
    for _ in 0..amount {
      self.action(DiscardNewCard(SingleCard::create(card_id)));
    }
  }
}

impl<'a, R: Runner> IntentEffectsContext for DoIntentContext<'a, R> {
  fn action(&mut self, action: impl Action) {
    self.runner.action_bottom(action)
  }
  fn state(&self) -> &CombatState {
    self.runner.state()
  }
  fn monster_index(&self) -> usize {
    self.monster_index
  }
  fn set_intent(&mut self, intent: impl Intent) {
    let index = self.monster_index();
    self.runner.state_mut().monsters[index]
      .move_history
      .push(intent.id());
  }
}

impl<'a, F: ConsiderAction> IntentEffectsContext for ConsiderIntentContext<'a, F> {
  fn action(&mut self, action: impl Action) {
    self.consider_action.consider(action)
  }
  fn state(&self) -> &CombatState {
    self.state
  }
  fn monster_index(&self) -> usize {
    self.monster_index
  }
  fn set_intent(&mut self, _intent: impl Intent) {}
}

impl<'a, R: Runner> DoIntentContext<'a, R> {
  pub fn new(runner: &'a mut R, monster_index: usize) -> Self {
    DoIntentContext {
      runner,
      monster_index,
    }
  }
}

pub fn consider_intent_actions(
  state: &CombatState,
  monster_index: usize,
  consider_action: &mut impl ConsiderAction,
) {
  let mut context = ConsiderIntentContext {
    state,
    monster_index,
    consider_action,
  };
  let monster_id = state.monsters[monster_index].monster_id;
  monster_id.intent_effects(&mut context);
}

impl CombatState {
  pub fn total_monster_attack_intent_damage(&self) -> i32 {
    struct CountAttackIntentDamage {
      total: i32,
    }
    impl ConsiderAction for CountAttackIntentDamage {
      fn consider(&mut self, action: impl Action) {
        // It theoretically makes more sense to do this on the type level, but that would make the code more complicated, and I'm almost certain this will be optimized out.
        if let DynAction::DamageAction(action) = action.clone().into() {
          self.total += action.info.output;
        }
      }
    }
    let mut counter = CountAttackIntentDamage { total: 0 };
    for (index, monster) in self.monsters.iter().enumerate() {
      if !monster.gone {
        consider_intent_actions(self, index, &mut counter);
      }
    }
    counter.total
  }
}

pub const MAX_INTENTS: usize = 7;

pub trait Intent: Enum<()> + Debug {
  fn id(self) -> IntentId {
    self.to_usize() as IntentId
  }
  fn from_id(intent_id: IntentId) -> Self {
    Self::from_usize(intent_id as usize)
  }
  fn from_communication_mod(intent_id: i32) -> Option<Self>;
}

pub trait MonsterBehavior: Sized + Copy + Into<MonsterId> {
  type Intent: Intent;
  fn make_intent_distribution(context: &mut IntentChoiceContext);

  fn after_choosing_intent(runner: &mut impl Runner, monster_index: usize) {}
  fn intent_effects(context: &mut impl IntentEffectsContext);
}

macro_rules! intent {
  (pub enum $Enum:ident {$($spire_id:tt: $Variant: ident,)*}) => {
    #[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Enum)]
    pub enum $Enum {
      $($Variant,)*
    }
    impl Intent for $Enum {
      fn from_communication_mod(intent_id: i32) -> Option<Self> {
        match intent_id {
          $($spire_id => Some($Enum::$Variant),)*
          _ => None,
        }
      }
    }
    impl From<$Enum> for IntentId {
      fn from(intent: $Enum)->IntentId {
        intent.id()
      }
    }
    impl From<$Enum> for Distribution<IntentId> {
      fn from(value: $Enum) -> Distribution<IntentId> {
        Distribution::from(value.id())
      }
    }
  };
}
macro_rules! monsters {
  ($([$id: expr, $Variant: ident],)*) => {
    #[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Debug)]
    pub enum MonsterId {
      $($Variant,)*
    }

    $(#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Debug)]
    pub struct $Variant;
    impl From<$Variant> for MonsterId {
      fn from (source: $Variant)->MonsterId {
        MonsterId::$Variant
      }
    }
    )*

    impl From<& str> for MonsterId {
      fn from (source: & str)->MonsterId {
        match source {
          $($id => MonsterId::$Variant,)*
          _ => MonsterId::Cultist,
        }
      }
    }

    impl MonsterId {
      pub fn intent_name(self, intent_id: IntentId) -> String {
        match self {
        $(MonsterId::$Variant => format!("{:?}", <<$Variant as MonsterBehavior>::Intent as Intent>::from_id (intent_id)),)*
        }
      }
      pub fn intent_from_communication_mod(self, spire_intent: i32) -> Option<IntentId> {
        match self {
        $(MonsterId::$Variant => <<$Variant as MonsterBehavior>::Intent as Intent>::from_communication_mod (spire_intent).map(Intent::id),)*
        }
      }
      pub fn make_intent_distribution (self, context: &mut IntentChoiceContext) {
        match self {
        $(MonsterId::$Variant => <$Variant as MonsterBehavior>::make_intent_distribution (context),)*
        }
      }
      pub fn after_choosing_intent (self, runner: &mut impl Runner, monster_index: usize) {
        match self {
        $(MonsterId::$Variant => <$Variant as MonsterBehavior>::after_choosing_intent (runner, monster_index),)*
        }
      }
      pub fn intent_effects(self, context: &mut impl IntentEffectsContext) {
        match self {
        $(MonsterId::$Variant => <$Variant as MonsterBehavior>::intent_effects(context),)*
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
  ["AcidSlime_L", AcidSlimeL],
  ["SpikeSlime_S", SpikeSlimeS],
  ["SpikeSlime_M", SpikeSlimeM],
  ["SpikeSlime_L", SpikeSlimeL],
  ["FungiBeast", FungiBeast],
  ["Looter", Looter],
  ["SlaverBlue", SlaverBlue],
  ["SlaverRed", SlaverRed],
  ["GremlinWarrior", MadGremlin],
  ["GremlinThief", SneakyGremlin],
  ["GremlinWizard", GremlinWizard],
  ["GremlinFat", FatGremlin],
  ["GremlinTsundere", ShieldGremlin],

  ["Sentry", Sentry],
  ["GremlinNob", GremlinNob],
  ["Lagavulin", Lagavulin],

  ["TheGuardian", TheGuardian],
  ["Hexaghost", Hexaghost],
  ["SlimeBoss", SlimeBoss],

  ["Byrd", Byrd],
}

mod beyond;
mod city;
mod ending;
mod exordium;
