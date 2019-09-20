#![allow (unused_variables)]

use serde::{Deserialize, Serialize};
use std::convert::From;

use crate::simulation::*;
use crate::simulation_state::*;

pub struct IntentChoiceContext<'a> {
  pub state: &'a CombatState,
  pub monster_index: usize,
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
  pub fn if_num_geq(&mut self, threshold: i32, value: impl Into<Distribution>) {
    // hack, assume that no function checks both greater and less
    self.if_num_lt(100 - threshold, value);
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
  pub fn state(&self) -> &CombatState {
    &self.state
  }
  pub fn monster_index(&self) -> usize {
    self.monster_index
  }

  fn creature_index(&self) -> CreatureIndex {
    CreatureIndex::Monster(self.monster_index())
  }
  fn monster(&self) -> &Monster {
    &self.state().monsters[self.monster_index()]
  }


  pub fn did_repeats(&self, repeats: Repeats, intent: i32) -> bool {
    self.monster.move_history.len() >= repeats.0
      && self.monster.move_history[self.monster.move_history.len() - repeats.0..]
        .iter()
        .all(|historical| *historical == intent)
  }
  pub fn with_max_repeats(
    &self,
    max_repeats: Repeats,
    intent: i32,
    alternative: impl Into<Distribution>,
  ) -> Distribution {
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
    monster_index,
    ascension: monster.ascension,
    num_distribution: Vec::new(),
  };
  monster_id.make_intent_distribution(&mut context);
  context.final_distribution()
}

pub struct DoIntentContext<'a, 'b> {
  pub runner: &'a mut Runner<'b>,
  pub monster_index: usize,
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

  fn attack(&mut self, base_damage: i32) {
    // hack: this is actually NOT where powers are applied to card/monster damage in the actual code
    let mut info = DamageInfo::new(self.creature_index(), base_damage, DamageType::Normal);
    info.apply_powers(
      self.state(),
      self.creature_index(),
      CreatureIndex::Player,
    );
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

  fn undefined_intent(&mut self) {}
}

impl<'a, 'b> IntentEffectsContext for DoIntentContext<'a, 'b> {
  fn action(&mut self, action: impl Action) {
    self.runner.action_bottom(action)
  }
  fn state(&self) -> &CombatState {
    self.runner.state()
  }
  fn monster_index(&self) -> usize {
    self.monster_index
  }
}

impl<'a, 'b> DoIntentContext<'a, 'b> {
  pub fn new(runner: &'a mut Runner<'b>, monster_index: usize) -> Self {
    DoIntentContext {
      runner,
      monster_index,
    }
  }
}

pub trait MonsterBehavior: Sized + Copy + Into<MonsterId> {
  fn make_intent_distribution(self, context: &mut IntentChoiceContext);

  fn after_choosing_intent(self, runner: &mut Runner, monster_index: usize) {}
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
      fn after_choosing_intent (self, runner: &mut Runner, monster_index: usize) {
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
}

impl MonsterBehavior for Cultist {
  fn make_intent_distribution(self, context: &mut IntentChoiceContext) {
    context.always(if context.first_move() { 3 } else { 1 });
  }
  fn intent_effects(self, context: &mut impl IntentEffectsContext) {
    match context.intent() {
      1 => context.attack(6),
      3 => context.power_self(
        PowerId::Ritual,
        context.with_ascensions(Ascension(17), 5, Ascension(2), 4, 3),
      ),
      _ => context.undefined_intent(),
    }
  }
}

impl MonsterBehavior for RedLouse {
  fn make_intent_distribution(self, context: &mut IntentChoiceContext) {
    let max_buff_repeats = Repeats(context.with_ascension(Ascension(17), 1, 2));
    context.if_num_lt(25, context.with_max_repeats(max_buff_repeats, 4, 3));
    context.else_num(context.with_max_repeats(Repeats(2), 3, 4));
  }
  fn after_choosing_intent(self, runner: &mut Runner, monster_index: usize) {
    if runner.state().monster_intent(monster_index) == 3 {
      let ascension = runner.state().monsters[monster_index].ascension;
      let bonus = if ascension >= 2 { 1 } else { 0 };
      runner.action_now (&InitializeMonsterInnateDamageAmount {
        monster_index,
        range: (5 + bonus, 8 + bonus),
      });
    }
  }
  fn intent_effects(self, context: &mut impl IntentEffectsContext) {
    match context.intent() {
      3 => context.attack(context.monster().innate_damage_amount.unwrap()),
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
  fn after_choosing_intent(self, runner: &mut Runner, monster_index: usize) {
    RedLouse.after_choosing_intent(runner, monster_index);
  }

  fn intent_effects(self, context: &mut impl IntentEffectsContext) {
    match context.intent() {
      3 => context.attack(context.monster().innate_damage_amount.unwrap()),
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
      context.with_max_repeats(Repeats(1), 1, Distribution::split(0.5625, 2, 3)),
    );
    context.if_num_lt(
      55,
      context.with_max_repeats(Repeats(2), 3, Distribution::split(0.357, 1, 2)),
    );
    context.else_num(context.with_max_repeats(Repeats(1), 1, Distribution::split(0.416, 1, 3)));
  }
  fn intent_effects(self, context: &mut impl IntentEffectsContext) {
    match context.intent() {
      1 => context.attack(context.with_ascension(Ascension(2), 12, 11)),
      2 => {
      context.power_self(
        PowerId::Strength,
        context.with_ascensions(Ascension(17), 5, Ascension(2), 4, 3),
      );
      context.block(context.with_ascension(Ascension(17), 9, 6));
      }
      3 => {
        context.attack(7);
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
      1 => context.attack(context.with_ascension(Ascension(2), 4, 3)),
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
        context.with_max_repeats(Repeats(2), 1, Distribution::split(0.5, 2, 4)),
      );
      context.if_num_lt(
        80,
        context.with_max_repeats(Repeats(2), 2, Distribution::split(0.5, 1, 4)),
      );
      context.else_num(context.with_max_repeats(Repeats(1), 4, Distribution::split(0.4, 1, 2)));
    } else {
      context.if_num_lt(
        30,
        context.with_max_repeats(Repeats(2), 1, Distribution::split(0.5, 2, 4)),
      );
      context.if_num_lt(
        70,
        context.with_max_repeats(Repeats(1), 2, Distribution::split(0.4, 1, 4)),
      );
      context.else_num(context.with_max_repeats(Repeats(2), 4, Distribution::split(0.4, 1, 2)));
    }
  }
  fn intent_effects(self, context: &mut impl IntentEffectsContext) {
    match context.intent() {
      1 => {
        context.attack(context.with_ascension(Ascension(2), 8, 7));
        context.discard_status(CardId::Slimed, 1);
      }
      2 => context.attack(context.with_ascension(Ascension(2), 12, 10)),
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
      1 => context.attack(context.with_ascension(Ascension(2), 6, 5)),
      _ => context.undefined_intent(),
    }
  }
}

impl MonsterBehavior for SpikeSlimeM {
  fn make_intent_distribution(self, context: &mut IntentChoiceContext) {
    let max_debuff_repeats = Repeats(context.with_ascension(Ascension(17), 1, 2));
    context.if_num_lt(30, context.with_max_repeats(Repeats(2), 1, 4));
    context.else_num(context.with_max_repeats(max_debuff_repeats, 4, 1));
  }
  fn intent_effects(self, context: &mut impl IntentEffectsContext) {
    match context.intent() {
      1 => {
        context.attack(context.with_ascension(Ascension(2), 10, 8));
        context.discard_status(CardId::Slimed, 1);
      }
      4 => context.power_player(PowerId::Frail, 1),
      _ => context.undefined_intent(),
    }
  }
}

impl MonsterBehavior for FungiBeast {
  fn make_intent_distribution(self, context: &mut IntentChoiceContext) {
    context.if_num_lt(60, context.with_max_repeats(Repeats(2), 1, 2));
    context.else_num(context.with_max_repeats(Repeats(1), 2, 1));
  }
  fn intent_effects(self, context: &mut impl IntentEffectsContext) {
    match context.intent() {
      1 => context.attack(6),
      2 => context.power_self(
        PowerId::Strength,
        context.with_ascension(Ascension(17), 4, 3),
      ),
      _ => context.undefined_intent(),
    }
  }
}
impl MonsterBehavior for Looter {
  fn make_intent_distribution(self, context: &mut IntentChoiceContext) {
    //TODO
    context.always(1);
  }
  fn intent_effects(self, context: &mut impl IntentEffectsContext) {
    match context.intent() {
      1 => context.attack(6),
      _ => context.undefined_intent(),
    }
  }
}
impl MonsterBehavior for SlaverBlue {
  fn make_intent_distribution(self, context: &mut IntentChoiceContext) {
    if !context.did_repeats(Repeats(2), 1) {
      context.if_num_geq(40, 1);
    }

    let max_rake_repeats = Repeats(context.with_ascension(Ascension(17), 1, 2));
    context.else_num(context.with_max_repeats(max_rake_repeats, 4, 1));
  }
  fn intent_effects(self, context: &mut impl IntentEffectsContext) {
    match context.intent() {
      1 => context.attack(context.with_ascension(Ascension(2), 13, 12)),
      4 => {
        context.attack(context.with_ascension(Ascension(2), 8, 7));
        context.power_player(PowerId::Weak, context.with_ascension(Ascension(17), 2, 1));
      }
      _ => context.undefined_intent(),
    }
  }
}
impl MonsterBehavior for SlaverRed {
  fn make_intent_distribution(self, context: &mut IntentChoiceContext) {
    if context.first_move() {
      context.always(1);
      return;
    }

    if context.monster().move_history.iter().all(| & intent | intent != 2) {
      context.if_num_geq(75, 2);
    } else if !context.did_repeats(Repeats(2), 1) {
      context.if_num_geq(55, 1);
    }

    let max_scrape_repeats = Repeats(context.with_ascension(Ascension(17), 1, 2));
    context.else_num(context.with_max_repeats(max_scrape_repeats, 3, 1));
  }
  fn intent_effects(self, context: &mut impl IntentEffectsContext) {
    match context.intent() {
      1 => context.attack(context.with_ascension(Ascension(2), 14, 13)),
      2 => context.power_player(PowerId::Entangled, 1),
      3 => {
        context.attack(context.with_ascension(Ascension(2), 9, 8));
        context.power_player(
          PowerId::Vulnerable,
          context.with_ascension(Ascension(17), 2, 1),
        );
      }
      _ => context.undefined_intent(),
    }
  }
}
impl MonsterBehavior for MadGremlin {
  fn make_intent_distribution(self, context: &mut IntentChoiceContext) {
    context.always(1);
  }
  fn intent_effects(self, context: &mut impl IntentEffectsContext) {
    match context.intent() {
      1 => context.attack(context.with_ascension(Ascension(2), 5, 4)),
      _ => context.undefined_intent(),
    }
  }
}
impl MonsterBehavior for SneakyGremlin {
  fn make_intent_distribution(self, context: &mut IntentChoiceContext) {
    context.always(1);
  }
  fn intent_effects(self, context: &mut impl IntentEffectsContext) {
    match context.intent() {
      1 => context.attack(context.with_ascension(Ascension(2), 10, 9)),
      _ => context.undefined_intent(),
    }
  }
}
impl MonsterBehavior for GremlinWizard {
  fn make_intent_distribution(self, context: &mut IntentChoiceContext) {
    if context.state().turn_number >= 3
      && (context.ascension() >= 17 || (context.state().turn_number % 4) == 3)
    {
      context.always(1);
    } else {
      context.always(2);
    }
  }
  fn intent_effects(self, context: &mut impl IntentEffectsContext) {
    match context.intent() {
      1 => context.attack(context.with_ascension(Ascension(2), 30, 25)),
      2 => (),
      _ => context.undefined_intent(),
    }
  }
}
impl MonsterBehavior for FatGremlin {
  fn make_intent_distribution(self, context: &mut IntentChoiceContext) {
    context.always(2);
  }
  fn intent_effects(self, context: &mut impl IntentEffectsContext) {
    match context.intent() {
      2 => {
        context.attack(context.with_ascension(Ascension(2), 5, 4));
        context.power_player(PowerId::Weak, 1);
        if context.ascension() >= 17 {
          context.power_player(PowerId::Frail, 1);
        }
      }
      _ => context.undefined_intent(),
    }
  }
}
impl MonsterBehavior for ShieldGremlin {
  fn make_intent_distribution(self, context: &mut IntentChoiceContext) {
    if context
      .state()
      .monsters
      .iter()
      .filter(|monster| !monster.gone)
      .count()
      > 1
    {
      context.always(1);
    } else {
      context.always(2);
    }
  }
  fn intent_effects(self, context: &mut impl IntentEffectsContext) {
    match context.intent() {
      1 => {
        let amount = context.with_ascensions(Ascension(17), 11, Ascension(2), 8, 7);
        //TODO
      }
      2 => context.attack(context.with_ascension(Ascension(2), 8, 6)),
      _ => context.undefined_intent(),
    }
  }
}
impl MonsterBehavior for Sentry {
  fn make_intent_distribution(self, context: &mut IntentChoiceContext) {
    if let Some(last_intent) = context.last_intent() {
      context.always(7 - last_intent);
    } else {
      if context.monster_index() % 2 == 0 {
        context.always(3);
      } else {
        context.always(4);
      }
    }
  }
  fn intent_effects(self, context: &mut impl IntentEffectsContext) {
    match context.intent() {
      3 => context.discard_status(CardId::Dazed, context.with_ascension(Ascension(18), 3, 2)),
      4 => context.attack(context.with_ascension(Ascension(3), 10, 9)),
      _ => context.undefined_intent(),
    }
  }
}
impl MonsterBehavior for GremlinNob {
  fn make_intent_distribution(self, context: &mut IntentChoiceContext) {
    if context.first_move() {
      context.always(3);
      return;
    }

    if context.ascension() >= 18 {
      if (context.state().turn_number % 3) == 2 {
        context.always(2);
      } else {
        context.always(1);
      }
    } else {
      context.if_num_lt(33, 2);
      context.else_num(context.with_max_repeats(Repeats(2), 1, 2));
    }
  }
  fn intent_effects(self, context: &mut impl IntentEffectsContext) {
    match context.intent() {
      1 => context.attack(context.with_ascension(Ascension(3), 16, 14)),
      2 => {
        context.attack(context.with_ascension(Ascension(3), 8, 6));
        context.power_player(PowerId::Vulnerable, 2);
      }
      3 => context.power_self(PowerId::Enrage, context.with_ascension(Ascension(18), 3, 2)),
      _ => context.undefined_intent(),
    }
  }
}
impl MonsterBehavior for Lagavulin {
  fn make_intent_distribution(self, context: &mut IntentChoiceContext) {
    if context.state().turn_number >= 3 || context.monster().creature.hitpoints < context.monster().creature.max_hitpoints || context.monster().move_history.iter().any(|&intent| intent == 1 || intent == 3) {
      context.always(context.with_max_repeats(Repeats(2), 3, 1));
    }
    else {
      context.always(5);
    }
  }
  fn after_choosing_intent(self, runner: &mut Runner, monster_index: usize) {
    let monster = &runner.state().monsters[monster_index];
    let intent = monster.intent();
    if intent == 1 || intent == 3 && monster.creature.power_amount (PowerId::Metallicize) >= 8 {
      runner.action_bottom (ReducePowerAction {target: CreatureIndex::Monster (monster_index), power_id: PowerId::Metallicize, amount: 8});
    }
  }
  fn intent_effects(self, context: &mut impl IntentEffectsContext) {
    match context.intent() {
      1 => {
        let amount = context.with_ascension(Ascension(18), -2, -1);
        context.power_player(PowerId::Dexterity, amount);
        context.power_player(PowerId::Strength, amount);
      }
      3 => context.attack(context.with_ascension(Ascension(3), 20, 18)),
      _ => context.undefined_intent(),
    }
  }
}
