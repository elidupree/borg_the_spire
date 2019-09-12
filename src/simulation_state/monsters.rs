#![allow (unused_variables)]

use serde::{Deserialize, Serialize};
use std::convert::From;

use crate::simulation::*;
use crate::simulation_state::*;

pub struct IntentChoiceContext<'a> {
  pub state: &'a CombatState,
  pub monster: &'a Monster,
  pub ascension: i32,
  num_distribution: Vec<(i32, Distribution)>,
}

impl<'a> IntentChoiceContext<'a> {
  pub fn if_num_lt(&mut self, threshold: i32, value: impl Into<Distribution>) {
    if threshold
      > self
        .num_distribution
        .last()
        .map_or(0, |(excluded, _)| *excluded)
    {
      self.num_distribution.push((threshold, value.into()));
    }
  }
  pub fn if_num_leq(&mut self, threshold: i32, value: impl Into<Distribution>) {
    self.if_num_lt(threshold + 1, value);
  }
  pub fn else_num(&mut self, value: impl Into<Distribution>) {
    self.if_num_lt(100, value);
  }
  pub fn always(&mut self, value: impl Into<Distribution>) {
    self.if_num_lt(100, value);
  }
  pub fn first_move(&self) -> bool {
    self.monster.move_history.is_empty()
  }
  pub fn last_intent(&self) -> Option<i32> {
    self.monster.move_history.last().cloned()
  }

  pub fn nonrepeating(
    &self,
    max_repeats: MaxRepeats,
    intent: i32,
    alternative: impl Into<Distribution>,
  ) -> Distribution {
    if self.monster.move_history.len() >= max_repeats.0
      && self.monster.move_history[self.monster.move_history.len() - max_repeats.0..]
        .iter()
        .all(|historical| *historical == intent)
    {
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

  pub fn final_distribution(self) -> Distribution {
    let mut start = 0;
    let mut result = Distribution::new();
    for (excluded, distribution) in self.num_distribution {
      result += distribution * ((excluded - start) as f64 / 100.0);
      start = excluded;
    }
    result
  }
}

pub fn intent_choice_distribution(state: &CombatState, monster_index: usize) -> Distribution {
  let state = state;
  let monster = &state.monsters[monster_index];

  let monster_id = monster.monster_id;
  let mut context = IntentChoiceContext {
    state,
    monster,
    ascension: monster.ascension,
    num_distribution: Vec::new(),
  };
  monster_id.make_intent_distribution(&mut context);
  context.final_distribution()
}

pub struct DoIntentContext<'a, R> {
  pub runner: &'a mut R,
  pub monster_index: usize,
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Debug)]
pub struct Ascension(pub i32);
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Debug)]
pub struct MaxRepeats(pub usize);

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
  fn intent(&self) -> i32 {
    self.monster().intent()
  }
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

  fn attack(&mut self, base_damage: i32, swings: i32) {
    self.action(AttackPlayer {
      monster_index: self.monster_index(),
      base_damage,
      swings,
    });
  }
  fn power_self(&mut self, power_id: PowerId, amount: i32) {
    self.action(ApplyPowerAmount {
      creature_index: self.creature_index(),
      power_id,
      amount,
      just_applied: true,
    });
  }
  fn power_player(&mut self, power_id: PowerId, amount: i32) {
    self.action(ApplyPowerAmount {
      creature_index: CreatureIndex::Player,
      power_id,
      amount,
      just_applied: true,
    });
  }
  fn block(&mut self, amount: i32) {
    self.action(Block {
      creature_index: self.creature_index(),
      amount,
    });
  }
  fn discard_status(&mut self, card_id: CardId, amount: i32) {
    self.action(DiscardNewCard(SingleCard::create(card_id)));
  }

  fn undefined_intent(&mut self) {}
}

impl<'a, R: Runner> IntentEffectsContext for DoIntentContext<'a, R> {
  fn action(&mut self, action: impl Action) {
    self.runner.apply(&action)
  }
  fn state(&self) -> &CombatState {
    self.runner.state()
  }
  fn monster_index(&self) -> usize {
    self.monster_index
  }
}

impl<'a, R: Runner> DoIntentContext<'a, R> {
  pub fn new(runner: &'a mut R, monster_index: usize) -> Self {
    DoIntentContext {
      runner,
      monster_index,
    }
  }
}

pub trait MonsterBehavior: Sized + Copy + Into<MonsterId> {
  fn make_intent_distribution(self, context: &mut IntentChoiceContext);

  fn after_choosing_intent(self, runner: &mut impl Runner, monster_index: usize) {}
  fn intent_effects(self, context: &mut impl IntentEffectsContext);
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

    impl MonsterBehavior for MonsterId {
      fn make_intent_distribution (self, context: &mut IntentChoiceContext) {
        match self {
        $(MonsterId::$Variant => $Variant.make_intent_distribution (context),)*
        }
      }
      fn after_choosing_intent (self, runner: &mut impl Runner, monster_index: usize) {
        match self {
        $(MonsterId::$Variant => $Variant.after_choosing_intent (runner, monster_index),)*
        }
      }
      fn intent_effects(self, context: &mut impl IntentEffectsContext) {
        match self {
        $(MonsterId::$Variant => $Variant.intent_effects(context),)*
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
  fn make_intent_distribution(self, context: &mut IntentChoiceContext) {
    context.always(if context.first_move() { 3 } else { 1 });
  }
  fn intent_effects(self, context: &mut impl IntentEffectsContext) {
    match context.intent() {
      1 => context.attack(context.with_ascension(Ascension(2), 12, 11), 1),
      3 => context.power_self(PowerId::Ritual, context.with_ascension(Ascension(17), 4, 3)),
      _ => context.undefined_intent(),
    }
  }
}

impl MonsterBehavior for RedLouse {
  fn make_intent_distribution(self, context: &mut IntentChoiceContext) {
    let max_buff_repeats = MaxRepeats(context.with_ascension(Ascension(17), 1, 2));
    context.if_num_lt(25, context.nonrepeating(max_buff_repeats, 4, 3));
    context.else_num(context.nonrepeating(MaxRepeats(2), 3, 4));
  }
  fn after_choosing_intent(self, runner: &mut impl Runner, monster_index: usize) {
    if runner.state().monster_intent(monster_index) == 3 {
      let ascension = runner.state().monsters[monster_index].ascension;
      let bonus = if ascension >= 2 { 1 } else { 0 };
      runner.apply(&InitializeMonsterInnateDamageAmount {
        monster_index,
        range: (5 + bonus, 8 + bonus),
      });
    }
  }
  fn intent_effects(self, context: &mut impl IntentEffectsContext) {
    match context.intent() {
      3 => context.attack(context.monster().innate_damage_amount.unwrap(), 1),
      4 => context.power_self(
        PowerId::Strength,
        context.with_ascension(Ascension(17), 4, 3),
      ),
      _ => context.undefined_intent(),
    }
  }
}

impl MonsterBehavior for GreenLouse {
  fn make_intent_distribution(self, context: &mut IntentChoiceContext) {
    RedLouse.make_intent_distribution(context);
  }
  fn after_choosing_intent(self, runner: &mut impl Runner, monster_index: usize) {
    RedLouse.after_choosing_intent(runner, monster_index);
  }

  fn intent_effects(self, context: &mut impl IntentEffectsContext) {
    match context.intent() {
      3 => context.attack(context.monster().innate_damage_amount.unwrap(), 1),
      4 => context.power_player(PowerId::Weak, 2),
      _ => context.undefined_intent(),
    }
  }
}

impl MonsterBehavior for JawWorm {
  fn make_intent_distribution(self, context: &mut IntentChoiceContext) {
    if context.first_move() {
      context.always(1);
    }
    context.if_num_lt(
      25,
      context.nonrepeating(MaxRepeats(1), 1, Distribution::split(0.5625, 2, 3)),
    );
    context.if_num_lt(
      55,
      context.nonrepeating(MaxRepeats(2), 3, Distribution::split(0.357, 1, 2)),
    );
    context.else_num(context.nonrepeating(MaxRepeats(1), 1, Distribution::split(0.416, 1, 3)));
  }
  fn intent_effects(self, context: &mut impl IntentEffectsContext) {
    match context.intent() {
      1 => context.attack(context.with_ascension(Ascension(2), 12, 11), 1),
      2 => context.power_self(
        PowerId::Strength,
        context.with_ascensions(Ascension(17), 5, Ascension(2), 4, 3),
      ),
      3 => {
        context.attack(7, 1);
        context.block(5);
      }
      _ => context.undefined_intent(),
    }
  }
}

impl MonsterBehavior for AcidSlimeS {
  fn make_intent_distribution(self, context: &mut IntentChoiceContext) {
    if let Some(last_intent) = context.last_intent() {
      context.always(3 - last_intent);
    } else if context.ascension() >= 17 {
      context.always(2);
    } else {
      context.always(Distribution::split(0.5, 1, 2));
    }
  }
  fn intent_effects(self, context: &mut impl IntentEffectsContext) {
    match context.intent() {
      1 => context.attack(context.with_ascension(Ascension(2), 4, 3), 1),
      2 => context.power_player(PowerId::Weak, 1),
      _ => context.undefined_intent(),
    }
  }
}

impl MonsterBehavior for AcidSlimeM {
  fn make_intent_distribution(self, context: &mut IntentChoiceContext) {
    if context.ascension() >= 17 {
      context.if_num_lt(
        40,
        context.nonrepeating(MaxRepeats(2), 1, Distribution::split(0.5, 2, 4)),
      );
      context.if_num_lt(
        80,
        context.nonrepeating(MaxRepeats(2), 2, Distribution::split(0.5, 1, 4)),
      );
      context.else_num(context.nonrepeating(MaxRepeats(1), 4, Distribution::split(0.4, 1, 2)));
    } else {
      context.if_num_lt(
        30,
        context.nonrepeating(MaxRepeats(2), 1, Distribution::split(0.5, 2, 4)),
      );
      context.if_num_lt(
        70,
        context.nonrepeating(MaxRepeats(1), 2, Distribution::split(0.4, 1, 4)),
      );
      context.else_num(context.nonrepeating(MaxRepeats(2), 4, Distribution::split(0.4, 1, 2)));
    }
  }
  fn intent_effects(self, context: &mut impl IntentEffectsContext) {
    match context.intent() {
      1 => {
        context.attack(context.with_ascension(Ascension(2), 8, 7), 1);
        context.discard_status(CardId::Slimed, 1);
      }
      2 => context.attack(context.with_ascension(Ascension(2), 12, 10), 1),
      4 => context.power_player(PowerId::Weak, 1),
      _ => context.undefined_intent(),
    }
  }
}

impl MonsterBehavior for SpikeSlimeS {
  fn make_intent_distribution(self, context: &mut IntentChoiceContext) {
    context.always(1);
  }
  fn intent_effects(self, context: &mut impl IntentEffectsContext) {
    match context.intent() {
      1 => context.attack(context.with_ascension(Ascension(2), 6, 5), 1),
      _ => context.undefined_intent(),
    }
  }
}

impl MonsterBehavior for SpikeSlimeM {
  fn make_intent_distribution(self, context: &mut IntentChoiceContext) {
    let max_debuff_repeats = MaxRepeats(context.with_ascension(Ascension(17), 1, 2));
    context.if_num_lt(30, context.nonrepeating(MaxRepeats(2), 1, 4));
    context.else_num(context.nonrepeating(max_debuff_repeats, 4, 1));
  }
  fn intent_effects(self, context: &mut impl IntentEffectsContext) {
    match context.intent() {
      1 => {
        context.attack(context.with_ascension(Ascension(2), 10, 8), 1);
        context.discard_status(CardId::Slimed, 1);
      }
      4 => context.power_player(PowerId::Frail, 1),
      _ => context.undefined_intent(),
    }
  }
}
