use serde::{Deserialize, Serialize};
use std::convert::From;

use crate::simulation_state::*;

macro_rules! powers {
  ($([$id: expr, $Variant: ident],)*) => {
    #[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Debug)]
    pub enum PowerId {
      $($Variant,)*
    }

    impl From<& str> for PowerId {
      fn from (source: & str)->PowerId {
        match source {
          $($id => PowerId::$Variant,)*
          _ => PowerId::Unknown,
        }
      }
    }
  }
}

powers! {
  ["Vulnerable", Vulnerable],
  ["Frail", Frail],
  ["Weakened", Weak],
  ["Strength", Strength],
  ["Dexterity", Dexterity],

  ["Ritual", Ritual],
  ["Curl Up", CurlUp],
  ["Thievery", Thievery],
  ["SporeCloud", SporeCloud],
  ["Entangled", Entangled],
  ["Angry", Angry],
  ["Anger", Enrage],

  ["Unknown", Unknown],
}
