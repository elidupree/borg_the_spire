use std::convert::From;
use serde::{Deserialize, Serialize};

use crate::simulation_state::*;

macro_rules! powers {
  ($([$id: expr, $Variant: ident],)*) => {
    #[derive(Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Debug)]
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
  ["Ritual", Ritual],
  ["Curl Up", CurlUp],
  ["Unknown", Unknown],
}
