#![allow (unused_variables)]

use serde::{Deserialize, Serialize};
use std::convert::From;

use crate::actions::*;
use crate::simulation::*;
use crate::simulation_state::*;

use self::CardType::{Attack, Curse, Power, Skill, Status};
use self::Rarity::{Basic, Common, Rare, Special, Uncommon};

pub trait CardBehavior: Sized + Copy + Into<CardId> {
  #[allow(unused)]
  fn behavior(self, context: &mut impl CardBehaviorContext) {}
}

pub trait CardBehaviorContext {
  fn action(&mut self, action: impl Action);
  fn target(&self) -> usize;
  fn attack_target(&mut self, base_damage: i32, swings: i32) {
    self.action(AttackMonster {
      target: self.target(),
      base_damage,
      swings,
    });
  }
  fn attack_monsters(&mut self, base_damage: i32, swings: i32) {
    self.action(AttackMonsters {
      base_damage,
      swings,
    });
  }
  fn power_monsters(&mut self, power_id: PowerId, amount: i32) {
    for index in 0..self.state().monsters.len() {
      self.action(ApplyPowerAmount {
        creature_index: CreatureIndex::Monster(index),
        power_id,
        amount,
        just_applied: false,
      });
    }
  }
  fn power_target(&mut self, power_id: PowerId, amount: i32) {
    self.action(ApplyPowerAmount {
      creature_index: CreatureIndex::Monster(self.target()),
      power_id,
      amount,
      just_applied: false,
    });
  }
  fn power_self(&mut self, power_id: PowerId, amount: i32) {
    self.action(ApplyPowerAmount {
      creature_index: CreatureIndex::Player,
      power_id,
      amount,
      just_applied: false,
    });
  }
  fn block(&mut self, amount: i32) {
    self.action(Block {
      creature_index: CreatureIndex::Player,
      amount,
    });
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

pub struct PlayCardContext<'a, R> {
  pub runner: &'a mut R,
  pub target: usize,
}

impl<'a, R: Runner> CardBehaviorContext for PlayCardContext<'a, R> {
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
    }
  }
}

pub const HAS_TARGET: bool = true;
pub const NO_TARGET: bool = false;

cards! {
  ["Strike_R", StrikeR, Attack, Basic, 1, HAS_TARGET, {}],
  ["Bash", Bash, Attack, Basic, 2, HAS_TARGET, {}],
  ["Defend_R", DefendR, Attack, Basic, 1, NO_TARGET, {}],
  ["Corruption", Corruption, Power, Uncommon, 3, NO_TARGET, {upgraded_cost: 2,}],
  ["Impervious", Impervious, Skill, Rare, 2, NO_TARGET, {}],
  ["Cleave", Cleave, Attack, Common, 1, NO_TARGET, {}],
  ["Injury", Injury, Curse, Special, -2, NO_TARGET, {}],
  ["AscendersBane", AscendersBane, Curse, Special, -2, NO_TARGET, {ethereal: true,}],
  ["Dazed", Dazed, Status, Special, -2, NO_TARGET, {ethereal: true,}],
  ["Slimed", Slimed, Status, Special, 1, NO_TARGET, {exhausts: true,}],
}

impl CardBehavior for StrikeR {
  fn behavior(self, context: &mut impl CardBehaviorContext) {
    context.attack_target(context.with_upgrade(9, 6), 1);
  }
}

impl CardBehavior for Bash {
  fn behavior(self, context: &mut impl CardBehaviorContext) {
    context.attack_target(context.with_upgrade(10, 8), 1);
    context.power_target(PowerId::Vulnerable, context.with_upgrade(3, 2));
  }
}

impl CardBehavior for DefendR {
  fn behavior(self, context: &mut impl CardBehaviorContext) {
    context.block(context.with_upgrade(8, 5));
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
impl CardBehavior for Cleave {
  fn behavior(self, context: &mut impl CardBehaviorContext) {
    context.attack_monsters(context.with_upgrade(11, 8), 1);
  }
}

impl CardBehavior for Injury {}
impl CardBehavior for AscendersBane {}
impl CardBehavior for Dazed {}
impl CardBehavior for Slimed {}
