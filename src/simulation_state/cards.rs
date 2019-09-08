#![allow (unused_variables)]

use serde::{Deserialize, Serialize};
use std::convert::From;

use crate::simulation::*;
use crate::simulation_state::*;

use self::CardType::{Attack, Curse, Power, Skill, Status};
use self::Rarity::{Basic, Common, Rare, Special, Uncommon};

pub trait CardBehavior: Sized {
  #[allow(unused)]
  fn play(self, state: &mut CombatState, runner: &mut impl Runner, target: usize) {}
}

macro_rules! cards {
  ($([$id: expr, $Variant: ident, $card_type: expr, $rarity: expr, $cost: expr, $has_target: expr, {$($type_info: tt)*}],)*) => {
    #[derive(Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Debug)]
    pub enum CardId {
      $($Variant,)*
    }

    $(#[derive(Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Debug)]
    pub struct $Variant;)*

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
      fn play (self, state: &mut CombatState, runner: &mut impl Runner, target: usize) {
        match self {
          $(CardId::$Variant => $Variant.play (state, runner, target),)*
        }
      }
    }
  }
}

cards! {
  ["Strike_R", StrikeR, Attack, Basic, 1, true, {}],
  ["Bash", Bash, Attack, Basic, 2, true, {}],
  ["Defend_R", DefendR, Attack, Basic, 1, false, {}],
  ["Corruption", Corruption, Power, Uncommon, 3, false, {upgraded_cost: 2,}],
  ["Impervious", Impervious, Skill, Rare, 2, false, {}],
  ["Cleave", Cleave, Attack, Common, 1, false, {}],
  ["Injury", Injury, Curse, Special, -2, false, {}],
  ["AscendersBane", AscendersBane, Curse, Special, -2, false, {ethereal: true,}],
  ["Dazed", Dazed, Status, Special, -2, false, {ethereal: true,}],
  ["Slimed", Slimed, Status, Special, 1, false, {exhausts: true,}],
}

impl CardBehavior for StrikeR {
  fn play(self, state: &mut CombatState, runner: &mut impl Runner, target: usize) {
    let card = state.card_in_play.clone().unwrap();
    state.player_attacks_monster(runner, target, if card.upgrades > 0 { 9 } else { 6 }, 1);
  }
}

impl CardBehavior for Bash {
  fn play(self, state: &mut CombatState, runner: &mut impl Runner, target: usize) {
    let card = state.card_in_play.clone().unwrap();
    state.player_attacks_monster(runner, target, if card.upgrades > 0 { 10 } else { 8 }, 1);
    state.monsters[target].creature.apply_power_amount(
      PowerId::Vulnerable,
      if card.upgrades > 0 { 3 } else { 2 },
      false,
    );
  }
}

impl CardBehavior for DefendR {
  fn play(self, state: &mut CombatState, _runner: &mut impl Runner, _target: usize) {
    let card = state.card_in_play.clone().unwrap();
    state
      .player
      .creature
      .do_block(if card.upgrades > 0 { 8 } else { 5 });
  }
}

impl CardBehavior for Corruption {
  fn play(self, state: &mut CombatState, _runner: &mut impl Runner, _target: usize) {}
}

impl CardBehavior for Impervious {
  fn play(self, state: &mut CombatState, _runner: &mut impl Runner, _target: usize) {
    let card = state.card_in_play.clone().unwrap();
    state
      .player
      .creature
      .do_block(if card.upgrades > 0 { 40 } else { 30 });
  }
}
impl CardBehavior for Cleave {
  fn play(self, state: &mut CombatState, runner: &mut impl Runner, _target: usize) {
    let card = state.card_in_play.clone().unwrap();
    state.player_attacks_all_monsters(runner, if card.upgrades > 0 { 11 } else { 8 }, 1);
  }
}

impl CardBehavior for Injury {}
impl CardBehavior for AscendersBane {}
impl CardBehavior for Dazed {}
impl CardBehavior for Slimed {}
