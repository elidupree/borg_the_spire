use serde::{Deserialize, Serialize};
use std::convert::From;

use crate::simulation_state::*;

use self::CardType::{Attack, Curse, Power, Skill, Status};
use self::Rarity::{Basic, Common, Rare, Special, Uncommon};

macro_rules! cards {
  ($([$id: expr, $Variant: ident, $card_type: expr, $rarity: expr, $cost: expr, $has_target: expr, {$($type_info: tt)*}],)*) => {
    #[derive(Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Debug)]
    pub enum CardId {
      $($Variant,)*
    }

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
  ["Slimed", Slimed, Status, Special, 1, false, {}],
}
