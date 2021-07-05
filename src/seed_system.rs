use derivative::Derivative;
use serde::{Deserialize, Serialize};
use smallvec::{smallvec, SmallVec};
use std::ops::{Add, AddAssign, Mul};

#[derive(Clone, PartialEq, Serialize, Deserialize, Debug, Derivative)]
pub struct Distribution(pub SmallVec<[(f64, i32); 4]>);

impl From<i32> for Distribution {
  fn from(value: i32) -> Distribution {
    Distribution(smallvec![(1.0, value)])
  }
}

impl Mul<f64> for Distribution {
  type Output = Distribution;
  fn mul(mut self, other: f64) -> Distribution {
    for pair in &mut self.0 {
      pair.0 *= other;
    }
    self
  }
}

impl Add<Distribution> for Distribution {
  type Output = Distribution;
  fn add(mut self, other: Distribution) -> Distribution {
    self += other;
    self
  }
}

impl AddAssign<Distribution> for Distribution {
  fn add_assign(&mut self, other: Distribution) {
    for (weight, value) in other.0 {
      if let Some(existing) = self
        .0
        .iter_mut()
        .find(|(_, existing_value)| *existing_value == value)
      {
        existing.0 += weight;
      } else {
        self.0.push((weight, value));
      }
    }
  }
}

impl Distribution {
  pub fn new() -> Distribution {
    Distribution(SmallVec::new())
  }
  pub fn split(
    probability: f64,
    then_value: impl Into<Distribution>,
    else_value: impl Into<Distribution>,
  ) -> Distribution {
    (then_value.into() * probability) + (else_value.into() * (1.0 - probability))
  }
}
