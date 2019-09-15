#![allow (unused_variables)]

use serde::{Deserialize, Serialize};
use std::convert::From;

//use crate::actions::*;
use crate::simulation::*;
use crate::simulation_state::*;

use self::CardType::{Attack, Curse, Power, Skill, Status};
use self::Rarity::{Basic, Common, Rare, Special, Uncommon};

pub trait CardBehavior: Sized + Copy + Into<CardId> {
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
    self.action(GainBlockAction {
      creature_index: CreatureIndex::Player,
      amount,
    });
  }
  fn draw_cards(&mut self, amount: i32) {
    self.action(DrawCards(amount));
  }
  fn state(&self) -> &CombatState;
  fn card(&self) -> &SingleCard {
    self.state().card_in_play.as_ref().unwrap()
  }
  fn with_upgrade<T>(&self, upgraded: T, normal: T) -> T {
    if self.card().upgrades > 0 {
      upgraded
    } else {
      normal
    }
  }
}

pub struct PlayCardContext<'a, 'b> {
  pub runner: &'a mut Runner<'b>,
  pub target: usize,
}

impl<'a, 'b> CardBehaviorContext for PlayCardContext<'a, 'b> {
  fn action(&mut self, action: impl Action) {
    self.runner.apply(&action);
  }
  fn target(&self) -> usize {
    self.target
  }
  fn state(&self) -> &CombatState {
    self.runner.state()
  }
}

macro_rules! cards {
  ($([$id: expr, $Variant: ident, $card_type: expr, $rarity: expr, $cost: expr, $has_target: expr, {$($type_info: tt)*}],)*) => {
    #[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Debug)]
    pub enum CardId {
      $($Variant,)*
    }

    $(#[derive(Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Debug)]
    pub struct $Variant;

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

    impl From <CardId> for CardInfo {
      fn from (source: CardId)->CardInfo {
        match source {
          $(CardId::$Variant => {
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
          },)*
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

  ["Corruption", Corruption, Power, Uncommon, 3, NO_TARGET, {upgraded_cost: 2,}],
  ["Impervious", Impervious, Skill, Rare, 2, NO_TARGET, {}],
  ["Injury", Injury, Curse, Special, -2, NO_TARGET, {}],
  ["AscendersBane", AscendersBane, Curse, Special, -2, NO_TARGET, {ethereal: true,}],
  ["Dazed", Dazed, Status, Special, -2, NO_TARGET, {ethereal: true,}],
  ["Slimed", Slimed, Status, Special, 1, NO_TARGET, {exhausts: true,}],
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
    // TODO: upgrades
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

impl CardBehavior for Corruption {
  fn behavior(self, context: &mut impl CardBehaviorContext) {}
}

impl CardBehavior for Impervious {
  fn behavior(self, context: &mut impl CardBehaviorContext) {
    context.block(context.with_upgrade(40, 30));
  }
}

impl CardBehavior for Injury {}
impl CardBehavior for AscendersBane {}
impl CardBehavior for Dazed {}
impl CardBehavior for Slimed {}
