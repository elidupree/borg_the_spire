#![allow(unused_variables)]

use enum_map::Enum;
use serde::{Deserialize, Serialize};
use std::convert::From;

//use crate::actions::*;
use crate::simulation::*;
use crate::simulation_state::*;

use self::CardType::{Attack, Curse, Power, Skill, Status};
use self::Rarity::{Basic, Common, Rare, Special, Uncommon};

pub trait CardSpecies: Sized + Copy + Into<CardId> + CardBehavior {
  const INFO: CardInfo;
}
pub trait CardBehavior: Sized {
  #[allow(unused)]
  fn behavior(self, context: &mut impl CardBehaviorContext) {}
  #[allow(unused)]
  fn playable(self, state: &CombatState) -> bool {
    true
  }
}

pub trait CardBehaviorContext {
  fn action(&mut self, action: impl Action);
  fn target(&self) -> usize;
  fn target_creature_index(&self) -> CreatureIndex {
    CreatureIndex::Monster(self.target())
  }
  fn attack_target(&mut self, base_damage: i32) {
    // hack: this is actually NOT where powers are applied to card/monster damage in the actual code
    let mut info = DamageInfo::new(CreatureIndex::Player, base_damage, DamageType::Normal);
    info.apply_powers(
      self.state(),
      CreatureIndex::Player,
      self.target_creature_index(),
    );
    self.action(DamageAction {
      target: self.target_creature_index(),
      info,
    });
  }
  fn attack_monsters(&mut self, base_damage: i32) {
    // hack: this is actually NOT where powers are applied to card/monster damage in the actual code
    let mut info = DamageInfo::new(CreatureIndex::Player, base_damage, DamageType::Normal);
    info.apply_powers(
      self.state(),
      CreatureIndex::Player,
      self.target_creature_index(),
    );
    self.action(DamageAllEnemiesAction {
      damage: info.output,
      damage_type: DamageType::Normal,
    });
  }
  fn power_monsters(&mut self, power_id: PowerId, amount: i32) {
    for index in 0..self.state().monsters.len() {
      self.action(ApplyPowerAction {
        source: CreatureIndex::Player,
        target: CreatureIndex::Monster(index),
        power_id,
        amount,
      });
    }
  }
  fn power_target(&mut self, power_id: PowerId, amount: i32) {
    self.action(ApplyPowerAction {
      source: CreatureIndex::Player,
      target: CreatureIndex::Monster(self.target()),
      power_id,
      amount,
    });
  }
  fn power_self(&mut self, power_id: PowerId, amount: i32) {
    self.action(ApplyPowerAction {
      source: CreatureIndex::Player,
      target: CreatureIndex::Player,
      power_id,
      amount,
    });
  }
  fn block(&mut self, amount: i32) {
    let mut amount = amount as f64;
    power_hook!(
      self.state(),
      CreatureIndex::Player,
      amount = modify_block(amount)
    );
    self.action(GainBlockAction {
      creature_index: CreatureIndex::Player,
      amount: amount as i32,
    });
  }
  fn draw_cards(&mut self, amount: i32) {
    self.action(DrawCards(amount));
  }
  fn state(&self) -> &CombatState;
  fn card(&self) -> &SingleCard {
    self.state().card_in_play.as_ref().unwrap()
  }
  fn upgraded(&self) -> bool {
    self.card().upgrades > 0
  }
  fn with_upgrade<T>(&self, upgraded: T, normal: T) -> T {
    if self.upgraded() {
      upgraded
    } else {
      normal
    }
  }
}

pub struct PlayCardContext<'a, R: Runner> {
  pub runner: &'a mut R,
  pub target: usize,
}

impl<'a, R: Runner> CardBehaviorContext for PlayCardContext<'a, R> {
  fn action(&mut self, action: impl Action) {
    self.runner.action_bottom(action);
  }
  fn target(&self) -> usize {
    self.target
  }
  fn state(&self) -> &CombatState {
    self.runner.state()
  }
}

pub struct ConsiderCardContext<'a, F> {
  pub state: &'a CombatState,
  pub target: usize,
  pub card: &'a SingleCard,
  pub consider_action: &'a mut F,
}

impl<'a, F: ConsiderAction> CardBehaviorContext for ConsiderCardContext<'a, F> {
  fn action(&mut self, action: impl Action) {
    self.consider_action.consider(action);
  }
  fn target(&self) -> usize {
    self.target
  }
  fn state(&self) -> &CombatState {
    self.state
  }
  fn card(&self) -> &SingleCard {
    self.card
  }
}

pub fn consider_card_actions(
  state: &CombatState,
  card: &SingleCard,
  target: usize,
  consider_action: &mut impl ConsiderAction,
) {
  let mut context = ConsiderCardContext {
    state,
    target,
    card,
    consider_action,
  };
  card.card_info.id.behavior(&mut context);
}
pub fn card_block_amount(state: &CombatState, card: &SingleCard, target: usize) -> i32 {
  struct CountBlock {
    total: i32,
  }
  impl ConsiderAction for CountBlock {
    fn consider(&mut self, action: impl Action) {
      // It theoretically makes more sense to do this on the type level, but that would make the code more complicated, and I'm almost certain this will be optimized out.
      if let DynAction::GainBlockAction(action) = action.clone().into() {
        self.total += action.amount;
      }
    }
  }
  let mut counter = CountBlock { total: 0 };
  consider_card_actions(state, card, target, &mut counter);
  counter.total
}

macro_rules! cards {
  ($([$id: expr, $Variant: ident, $card_type: expr, $rarity: expr, $cost: expr, $has_target: expr, {$($type_info: tt)*}],)*) => {
    #[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Enum, Debug)]
    pub enum CardId {
      $($Variant,)*
    }

    $(#[derive(Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Debug)]
    pub struct $Variant;

    impl CardSpecies for $Variant {
      const INFO: CardInfo = {
        let mut result = CardInfo {
          id: CardId::$Variant,
          card_type: $card_type,
          rarity: $rarity,
          normal_cost: $cost,
          has_target: $has_target,
          $($type_info)*
          .. CardInfo::default()
        };
        if result.upgraded_cost == -3 {result.upgraded_cost = result.normal_cost;}
        result
      };
    }

    impl From<$Variant> for CardId {
      fn from (source: $Variant)->CardId {
        CardId::$Variant
      }
    }
)*

    impl From<& str> for CardId {
      fn from (source: & str)->CardId {
        match source {
          $($id => CardId::$Variant,)*
          _ => CardId::Injury,
        }
      }
    }

    impl From <CardId> for &'static CardInfo {
      fn from (source: CardId)->&'static CardInfo {
        match source {
          $(CardId::$Variant => &$Variant::INFO,)*
        }
      }
    }

    impl CardBehavior for CardId {
      fn behavior(self, context: &mut impl CardBehaviorContext){
        match self {
          $(CardId::$Variant => $Variant.behavior (context),)*
        }
      }
      fn playable(self, state: &CombatState) -> bool {
        match self {
          $(CardId::$Variant => $Variant.playable(state),)*
        }
      }
    }
  }
}

pub const HAS_TARGET: bool = true;
pub const NO_TARGET: bool = false;

cards! {
  ["Strike_R", StrikeR, Attack, Basic, 1, HAS_TARGET, {}],
  ["Bash", Bash, Attack, Basic, 2, HAS_TARGET, {}],
  ["Defend_R", DefendR, Skill, Basic, 1, NO_TARGET, {}],

  ["Anger", Anger, Attack, Common, 0, HAS_TARGET, {}],
  ["Armaments", Armaments, Skill, Common, 1, NO_TARGET, {}],
  ["Body Slam", BodySlam, Attack, Common, 1, HAS_TARGET, {upgraded_cost: 0,}],
  ["Clash", Clash, Attack, Common, 0, HAS_TARGET, {}],
  ["Cleave", Cleave, Attack, Common, 1, NO_TARGET, {}],
  ["Clothesline", Clothesline, Attack, Common, 2, HAS_TARGET, {}],
  ["Flex", Flex, Skill, Common, 0, NO_TARGET, {}],
  ["Havoc", Havoc, Skill, Common, 1, NO_TARGET, {upgraded_cost: 0,}],
  ["Headbutt", Headbutt, Attack, Common, 1, HAS_TARGET, {}],
  ["Heavy Blade", HeavyBlade, Attack, Common, 2, HAS_TARGET, {}],
  ["Iron Wave", IronWave, Attack, Common, 1, HAS_TARGET, {}],
  ["Perfected Strike", PerfectedStrike, Attack, Common, 2, HAS_TARGET, {}],
  ["Pommel Strike", PommelStrike, Attack, Common, 1, HAS_TARGET, {}],
  ["Shrug It Off", ShrugItOff, Skill, Common, 1, NO_TARGET, {}],
  ["Sword Boomerang", SwordBoomerang, Attack, Common, 1, NO_TARGET, {}],
  ["Thunderclap", Thunderclap, Attack, Common, 1, NO_TARGET, {}],
  ["True Grit", TrueGrit, Skill, Common, 1, NO_TARGET, {}],
  ["Twin Strike", TwinStrike, Attack, Common, 1, HAS_TARGET, {}],
  ["Warcry", Warcry, Skill, Common, 0, NO_TARGET, {}],
  ["Wild Strike", WildStrike, Attack, Common, 1, HAS_TARGET, {}],

  ["Battle Trance", BattleTrance, Skill, Uncommon, 0, NO_TARGET, {}],
  ["Blood for Blood", BloodForBlood, Attack, Uncommon, 4, HAS_TARGET, {upgraded_cost: 3,}],
  ["Bloodletting", Bloodletting, Skill, Uncommon, 0, NO_TARGET, {}],
  ["Burning Pact", BurningPact, Skill, Uncommon, 1, NO_TARGET, {}],
  ["Carnage", Carnage, Attack, Uncommon, 2, HAS_TARGET, {ethereal: true,}],
  ["Combust", Combust, Power, Uncommon, 1, NO_TARGET, {}],
  ["Corruption", Corruption, Power, Uncommon, 3, NO_TARGET, {upgraded_cost: 2,}],
  ["Disarm", Disarm, Skill, Uncommon, 1, HAS_TARGET, {exhausts: true,}],
  ["Dropkick", Dropkick, Attack, Uncommon, 1, HAS_TARGET, {}],
  ["Dual Wield", DualWield, Skill, Uncommon, 1, NO_TARGET, {}],
  ["Entrench", Entrench, Skill, Uncommon, 2, NO_TARGET, {upgraded_cost: 1,}],
  ["Evolve", Evolve, Power, Uncommon, 1, NO_TARGET, {}],
  ["Feel No Pain", FeelNoPain, Power, Uncommon, 1, NO_TARGET, {}],
  ["Fire Breathing", FireBreathing, Power, Uncommon, 1, NO_TARGET, {upgraded_cost: 0,}],
  ["Flame Barrier", FlameBarrier, Skill, Uncommon, 2, NO_TARGET, {}],
  ["Ghostly Armor", GhostlyArmor, Skill, Uncommon, 1, NO_TARGET, {ethereal: true,}],
  ["Hemokinesis", Hemokinesis, Attack, Uncommon, 1, HAS_TARGET, {}],
  ["Infernal Blade", InfernalBlade, Skill, Uncommon, 1, NO_TARGET, {}],
  ["Inflame", Inflame, Power, Uncommon, 1, NO_TARGET, {}],
  ["Intimidate", Intimidate, Skill, Uncommon, 0, NO_TARGET, {exhausts: true,}],
  ["Metallicize", Metallicize, Power, Uncommon, 1, NO_TARGET, {}],
  ["Power Through", PowerThrough, Skill, Uncommon, 1, NO_TARGET, {}],
  ["Pummel", Pummel, Attack, Uncommon, 1, HAS_TARGET, {exhausts: true,}],
  ["Rage", Rage, Skill, Uncommon, 0, NO_TARGET, {}],
  ["Rampage", Rampage, Attack, Uncommon, 1, HAS_TARGET, {}],
  ["Reckless Charge", RecklessCharge, Attack, Uncommon, 0, HAS_TARGET, {}],
  ["Rupture", Rupture, Power, Uncommon, 1, NO_TARGET, {upgraded_cost: 0,}],
  ["Searing Blow", SearingBlow, Attack, Uncommon, 2, HAS_TARGET, {}],
  ["Second Wind", SecondWind, Skill, Uncommon, 1, NO_TARGET, {}],
  ["Seeing Red", SeeingRed, Skill, Uncommon, 1, NO_TARGET, {exhausts: true,}],
  ["Sentinel", Sentinel, Skill, Uncommon, 1, NO_TARGET, {}],
  ["Sever Soul", SeverSoul, Attack, Uncommon, 2, HAS_TARGET, {}],
  ["Shockwave", Shockwave, Skill, Uncommon, 2, NO_TARGET, {}],
  ["Spot Weakness", SpotWeakness, Skill, Uncommon, 1, HAS_TARGET, {}],
  ["Uppercut", Uppercut, Attack, Uncommon, 2, HAS_TARGET, {}],
  ["Whirlwind", Whirlwind, Attack, Uncommon, X_COST, HAS_TARGET, {}],

  ["Barricade", Barricade, Power, Rare, 3, NO_TARGET, {upgraded_cost: 2,}],
  ["Berserk", Berserk, Power, Rare, 0, NO_TARGET, {}],
  ["Bludgeon", Bludgeon, Attack, Rare, 3, HAS_TARGET, {}],
  ["Brutality", Brutality, Power, Rare, 0, NO_TARGET, {}],
  ["Dark Embrace", DarkEmbrace, Power, Rare, 2, NO_TARGET, {upgraded_cost: 1,}],
  ["Demon Form", DemonForm, Power, Rare, 3, NO_TARGET, {}],
  ["Double Tap", DoubleTap, Skill, Rare, 1, NO_TARGET, {}],
  ["Immolate", Immolate, Attack, Rare, 2, NO_TARGET, {}],
  ["Impervious", Impervious, Skill, Rare, 2, NO_TARGET, {exhausts: true,}],
  ["Juggernaut", Juggernaut, Power, Rare, 2, NO_TARGET, {}],

  ["Injury", Injury, Curse, Special, UNPLAYABLE, NO_TARGET, {}],
  ["AscendersBane", AscendersBane, Curse, Special, UNPLAYABLE, NO_TARGET, {ethereal: true,}],
  ["Dazed", Dazed, Status, Special, UNPLAYABLE, NO_TARGET, {ethereal: true,}],
  ["Slimed", Slimed, Status, Special, 1, NO_TARGET, {exhausts: true,}],
  ["Burn", Burn, Status, Special, UNPLAYABLE, NO_TARGET, {}],
}

impl CardBehavior for StrikeR {
  fn behavior(self, context: &mut impl CardBehaviorContext) {
    context.attack_target(context.with_upgrade(9, 6));
  }
}

impl CardBehavior for Bash {
  fn behavior(self, context: &mut impl CardBehaviorContext) {
    context.attack_target(context.with_upgrade(10, 8));
    context.power_target(PowerId::Vulnerable, context.with_upgrade(3, 2));
  }
}

impl CardBehavior for DefendR {
  fn behavior(self, context: &mut impl CardBehaviorContext) {
    context.block(context.with_upgrade(8, 5));
  }
}

impl CardBehavior for Anger {
  fn behavior(self, context: &mut impl CardBehaviorContext) {
    context.attack_target(context.with_upgrade(8, 6));
    context.action(DiscardNewCard(context.card().clone()));
  }
}

impl CardBehavior for Armaments {
  fn behavior(self, context: &mut impl CardBehaviorContext) {
    context.block(5);
    context.action(ArmamentsAction {
      upgraded: context.upgraded(),
    });
  }
}

impl CardBehavior for BodySlam {
  fn behavior(self, context: &mut impl CardBehaviorContext) {
    context.attack_target(context.state().player.creature.block);
  }
}

impl CardBehavior for Clash {
  fn playable(self, state: &CombatState) -> bool {
    state
      .hand
      .iter()
      .all(|card| card.card_info.card_type == Attack)
  }
  fn behavior(self, context: &mut impl CardBehaviorContext) {
    context.attack_target(context.with_upgrade(18, 14));
  }
}

impl CardBehavior for Cleave {
  fn behavior(self, context: &mut impl CardBehaviorContext) {
    context.attack_monsters(context.with_upgrade(11, 8));
  }
}

impl CardBehavior for Clothesline {
  fn behavior(self, context: &mut impl CardBehaviorContext) {
    context.attack_target(context.with_upgrade(14, 12));
    context.power_target(PowerId::Weak, context.with_upgrade(3, 2));
  }
}

impl CardBehavior for Flex {
  fn behavior(self, context: &mut impl CardBehaviorContext) {
    context.power_self(PowerId::Strength, context.with_upgrade(4, 2));
    //TODO: strength down
  }
}

impl CardBehavior for Havoc {
  fn behavior(self, context: &mut impl CardBehaviorContext) {
    //TODO
  }
}

impl CardBehavior for Headbutt {
  fn behavior(self, context: &mut impl CardBehaviorContext) {
    context.attack_target(context.with_upgrade(12, 9));
    //TODO: retrieve card
  }
}

impl CardBehavior for HeavyBlade {
  fn behavior(self, context: &mut impl CardBehaviorContext) {
    let multiplier = context.with_upgrade(4, 2);
    let strength = context
      .state()
      .player
      .creature
      .power_amount(PowerId::Strength);
    context.attack_target(14 + strength * multiplier);
  }
}

impl CardBehavior for IronWave {
  fn behavior(self, context: &mut impl CardBehaviorContext) {
    context.attack_target(context.with_upgrade(7, 5));
    context.block(context.with_upgrade(7, 5));
  }
}

impl CardBehavior for PerfectedStrike {
  fn behavior(self, context: &mut impl CardBehaviorContext) {
    //TODO
  }
}

impl CardBehavior for PommelStrike {
  fn behavior(self, context: &mut impl CardBehaviorContext) {
    context.attack_target(context.with_upgrade(10, 9));
    context.draw_cards(context.with_upgrade(2, 1));
  }
}

impl CardBehavior for ShrugItOff {
  fn behavior(self, context: &mut impl CardBehaviorContext) {
    context.block(context.with_upgrade(11, 8));
    context.draw_cards(1);
  }
}

impl CardBehavior for SwordBoomerang {
  fn behavior(self, context: &mut impl CardBehaviorContext) {
    //TODO
  }
}

impl CardBehavior for Thunderclap {
  fn behavior(self, context: &mut impl CardBehaviorContext) {
    context.attack_monsters(context.with_upgrade(7, 4));
    context.power_monsters(PowerId::Vulnerable, 1);
  }
}

impl CardBehavior for TrueGrit {
  fn behavior(self, context: &mut impl CardBehaviorContext) {
    context.block(context.with_upgrade(9, 7));
    //TODO: exhaust
  }
}

impl CardBehavior for TwinStrike {
  fn behavior(self, context: &mut impl CardBehaviorContext) {
    for _ in 0..2 {
      context.attack_target(context.with_upgrade(7, 5));
    }
  }
}

impl CardBehavior for Warcry {
  fn behavior(self, context: &mut impl CardBehaviorContext) {
    //TODO
  }
}

impl CardBehavior for WildStrike {
  fn behavior(self, context: &mut impl CardBehaviorContext) {
    context.attack_target(context.with_upgrade(17, 12));
    //TODO: wound
  }
}

impl CardBehavior for BattleTrance {
  fn behavior(self, context: &mut impl CardBehaviorContext) {
    context.draw_cards(context.with_upgrade(4, 3));
    context.power_self(PowerId::NoDraw, -1);
  }
}

impl CardBehavior for BloodForBlood {
  fn behavior(self, context: &mut impl CardBehaviorContext) {
    context.attack_target(context.with_upgrade(22, 18));
    //TODO: wound
  }
}

impl CardBehavior for Bloodletting {
  fn behavior(self, context: &mut impl CardBehaviorContext) {
    //TODO
  }
}

impl CardBehavior for BurningPact {
  fn behavior(self, context: &mut impl CardBehaviorContext) {
    //TODO
  }
}

impl CardBehavior for Carnage {
  fn behavior(self, context: &mut impl CardBehaviorContext) {
    context.attack_target(context.with_upgrade(28, 20));
  }
}

impl CardBehavior for Combust {
  fn behavior(self, context: &mut impl CardBehaviorContext) {
    context.power_self(PowerId::Combust, context.with_upgrade(7, 5));
  }
}

impl CardBehavior for Corruption {
  fn behavior(self, context: &mut impl CardBehaviorContext) {
    context.power_self(PowerId::Corruption, -1);
  }
}

impl CardBehavior for Disarm {
  fn behavior(self, context: &mut impl CardBehaviorContext) {
    context.power_target(PowerId::Strength, context.with_upgrade(-3, -2));
  }
}

impl CardBehavior for Dropkick {
  fn behavior(self, context: &mut impl CardBehaviorContext) {
    context.attack_target(context.with_upgrade(8, 5));
    // TODO conditional effect
  }
}

impl CardBehavior for DualWield {
  fn behavior(self, context: &mut impl CardBehaviorContext) {
    //TODO
  }
}

impl CardBehavior for Entrench {
  fn behavior(self, context: &mut impl CardBehaviorContext) {
    //TODO
  }
}

impl CardBehavior for Evolve {
  fn behavior(self, context: &mut impl CardBehaviorContext) {
    context.power_self(PowerId::Evolve, context.with_upgrade(2, 1));
  }
}

impl CardBehavior for FeelNoPain {
  fn behavior(self, context: &mut impl CardBehaviorContext) {
    context.power_self(PowerId::FeelNoPain, context.with_upgrade(4, 3));
  }
}

impl CardBehavior for FireBreathing {
  fn behavior(self, context: &mut impl CardBehaviorContext) {
    context.power_self(PowerId::FireBreathing, 1);
  }
}

impl CardBehavior for FlameBarrier {
  fn behavior(self, context: &mut impl CardBehaviorContext) {
    context.block(context.with_upgrade(16, 12));
    context.power_self(PowerId::FlameBarrier, context.with_upgrade(6, 4));
  }
}

impl CardBehavior for GhostlyArmor {
  fn behavior(self, context: &mut impl CardBehaviorContext) {
    context.block(context.with_upgrade(13, 10));
  }
}

impl CardBehavior for Hemokinesis {
  fn behavior(self, context: &mut impl CardBehaviorContext) {
    context.attack_target(context.with_upgrade(18, 14));
    // TODO other effect
  }
}

impl CardBehavior for InfernalBlade {
  fn behavior(self, context: &mut impl CardBehaviorContext) {
    //TODO
  }
}

impl CardBehavior for Inflame {
  fn behavior(self, context: &mut impl CardBehaviorContext) {
    context.power_self(PowerId::Strength, context.with_upgrade(3, 2));
  }
}

impl CardBehavior for Intimidate {
  fn behavior(self, context: &mut impl CardBehaviorContext) {
    context.power_monsters(PowerId::Weak, context.with_upgrade(2, 1));
  }
}

impl CardBehavior for Metallicize {
  fn behavior(self, context: &mut impl CardBehaviorContext) {
    context.power_self(PowerId::Metallicize, context.with_upgrade(4, 3));
  }
}

impl CardBehavior for PowerThrough {
  fn behavior(self, context: &mut impl CardBehaviorContext) {
    context.block(context.with_upgrade(20, 15));
    //TODO wounds
  }
}

impl CardBehavior for Pummel {
  fn behavior(self, context: &mut impl CardBehaviorContext) {
    for _ in 0..context.with_upgrade(5, 4) {
      context.attack_target(2);
    }
  }
}

impl CardBehavior for Rage {
  fn behavior(self, context: &mut impl CardBehaviorContext) {
    context.power_self(PowerId::Rage, context.with_upgrade(5, 3));
  }
}

impl CardBehavior for Rampage {
  fn behavior(self, context: &mut impl CardBehaviorContext) {
    context.attack_target(8);
    // TODO other effect
  }
}

impl CardBehavior for RecklessCharge {
  fn behavior(self, context: &mut impl CardBehaviorContext) {
    context.attack_target(context.with_upgrade(10, 7));
    // TODO other effect
  }
}

impl CardBehavior for Rupture {
  fn behavior(self, context: &mut impl CardBehaviorContext) {
    //TODO
  }
}

impl CardBehavior for SearingBlow {
  fn behavior(self, context: &mut impl CardBehaviorContext) {
    context.attack_target(context.with_upgrade(16, 12));
    // TODO further scaling
  }
}

impl CardBehavior for SecondWind {
  fn behavior(self, context: &mut impl CardBehaviorContext) {
    //TODO
  }
}

impl CardBehavior for SeeingRed {
  fn behavior(self, context: &mut impl CardBehaviorContext) {
    //TODO
  }
}

impl CardBehavior for SeverSoul {
  fn behavior(self, context: &mut impl CardBehaviorContext) {
    context.attack_target(context.with_upgrade(20, 16));
    // TODO other effect
  }
}

impl CardBehavior for Sentinel {
  fn behavior(self, context: &mut impl CardBehaviorContext) {
    context.block(context.with_upgrade(8, 5));
    //TODO
  }
}

impl CardBehavior for Shockwave {
  fn behavior(self, context: &mut impl CardBehaviorContext) {
    let amount = context.with_upgrade(5, 3);
    context.power_monsters(PowerId::Weak, amount);
    context.power_monsters(PowerId::Vulnerable, amount);
  }
}

impl CardBehavior for SpotWeakness {
  fn behavior(self, context: &mut impl CardBehaviorContext) {
    //TODO
  }
}

impl CardBehavior for Uppercut {
  fn behavior(self, context: &mut impl CardBehaviorContext) {
    context.attack_target(13);
    let amount = context.with_upgrade(2, 1);
    context.power_target(PowerId::Weak, amount);
    context.power_target(PowerId::Vulnerable, amount);
  }
}

impl CardBehavior for Whirlwind {
  fn behavior(self, context: &mut impl CardBehaviorContext) {
    //TODO
  }
}

impl CardBehavior for Barricade {
  fn behavior(self, context: &mut impl CardBehaviorContext) {
    context.power_self(PowerId::Barricade, -1);
  }
}

impl CardBehavior for Berserk {
  fn behavior(self, context: &mut impl CardBehaviorContext) {
    context.power_self(PowerId::Berserk, 1);
    context.power_self(PowerId::Vulnerable, context.with_upgrade(1, 2));
  }
}

impl CardBehavior for Bludgeon {
  fn behavior(self, context: &mut impl CardBehaviorContext) {
    context.attack_target(context.with_upgrade(42, 32));
  }
}

impl CardBehavior for Brutality {
  fn behavior(self, context: &mut impl CardBehaviorContext) {
    context.power_self(PowerId::Brutality, 1);
  }
}

impl CardBehavior for DarkEmbrace {
  fn behavior(self, context: &mut impl CardBehaviorContext) {
    context.power_self(PowerId::DarkEmbrace, 1);
  }
}

impl CardBehavior for DemonForm {
  fn behavior(self, context: &mut impl CardBehaviorContext) {
    context.power_self(PowerId::DemonForm, context.with_upgrade(3, 2));
  }
}

impl CardBehavior for DoubleTap {
  fn behavior(self, context: &mut impl CardBehaviorContext) {
    context.power_self(PowerId::DoubleTap, context.with_upgrade(2, 1));
  }
}

impl CardBehavior for Immolate {
  fn behavior(self, context: &mut impl CardBehaviorContext) {
    context.attack_target(context.with_upgrade(28, 21));
    context.action(DiscardNewCard(SingleCard::create(CardId::Burn)));
  }
}

impl CardBehavior for Impervious {
  fn behavior(self, context: &mut impl CardBehaviorContext) {
    context.block(context.with_upgrade(40, 30));
  }
}

impl CardBehavior for Juggernaut {
  fn behavior(self, context: &mut impl CardBehaviorContext) {
    context.power_self(PowerId::Juggernaut, context.with_upgrade(7, 5));
  }
}

impl CardBehavior for Injury {}
impl CardBehavior for AscendersBane {}
impl CardBehavior for Dazed {}
impl CardBehavior for Slimed {}
impl CardBehavior for Burn {}
