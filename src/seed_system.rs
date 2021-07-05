use derivative::Derivative;
use ordered_float::NotNan;
use rand::{Rng, SeedableRng};
use rand_pcg::Pcg64Mcg;
use serde::{Deserialize, Serialize};
use smallvec::{smallvec, SmallVec};
use std::cell::RefCell;
use std::collections::HashMap;
use std::hash::Hash;
use std::ops::{Add, AddAssign, Mul};
use std::rc::Rc;

#[derive(Clone, PartialEq, Serialize, Deserialize, Debug, Derivative)]
pub struct Distribution<Choice>(pub SmallVec<[(f64, Choice); 4]>);

impl<Choice> From<Choice> for Distribution<Choice> {
  fn from(value: Choice) -> Distribution<Choice> {
    Distribution(smallvec![(1.0, value)])
  }
}

impl<Choice> Mul<f64> for Distribution<Choice> {
  type Output = Distribution<Choice>;
  fn mul(mut self, other: f64) -> Distribution<Choice> {
    for pair in &mut self.0 {
      pair.0 *= other;
    }
    self
  }
}

impl<Choice: PartialEq + AddAssign<Choice>> Add<Distribution<Choice>> for Distribution<Choice> {
  type Output = Distribution<Choice>;
  fn add(mut self, other: Distribution<Choice>) -> Distribution<Choice> {
    self += other;
    self
  }
}

impl<Choice: PartialEq + AddAssign<Choice>> AddAssign<Distribution<Choice>>
  for Distribution<Choice>
{
  fn add_assign(&mut self, other: Distribution<Choice>) {
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

impl<Choice> Distribution<Choice> {
  pub fn new() -> Distribution<Choice> {
    Distribution(SmallVec::new())
  }
}
impl<Choice: PartialEq + AddAssign<Choice>> Distribution<Choice> {
  pub fn split(
    probability: f64,
    then_value: impl Into<Distribution<Choice>>,
    else_value: impl Into<Distribution<Choice>>,
  ) -> Distribution<Choice> {
    (then_value.into() * probability) + (else_value.into() * (1.0 - probability))
  }
}

pub trait GameState {
  type RandomForkType;
  type RandomChoice: Clone;
}

pub trait ChoiceLineageIdentity<G: GameState> {
  fn get(state: &G, fork_type: &G::RandomForkType, choice: &G::RandomChoice) -> Self;
}

pub trait SeedView<G: GameState>: Clone {
  type ChoiceLineageIdentity: ChoiceLineageIdentity<G>;
  fn gen(&mut self, identity: Self::ChoiceLineageIdentity) -> f64;
}

/// The presence or absence of a non-chosen choice has no effect on which choice is chosen,
/// and the presence or absence of ANY choice has no effect on how many times `seed.gen()` is called for any other choice.
pub fn choose_choice<G: GameState, S: SeedView<G>>(
  state: &G,
  fork_type: &G::RandomForkType,
  distribution: &Distribution<G::RandomChoice>,
  seed: &mut S,
) -> G::RandomChoice {
  distribution
    .0
    .iter()
    .min_by_key(|&(weight, choice)| {
      let identity = S::ChoiceLineageIdentity::get(state, fork_type, choice);
      let value = seed.gen(identity);
      NotNan::new(value / weight).unwrap()
    })
    .unwrap()
    .1
    .clone()
}

impl<G: GameState> ChoiceLineageIdentity<G> for () {
  fn get(_state: &G, _fork_type: &G::RandomForkType, _choice: &G::RandomChoice) -> Self {
    ()
  }
}

#[derive(Clone, Debug, Default, Derivative)]
pub struct Unseeded;

impl<G: GameState> SeedView<G> for Unseeded {
  type ChoiceLineageIdentity = ();

  fn gen(&mut self, _identity: ()) -> f64 {
    rand::random()
  }
}

#[derive(Clone, Debug)]
pub struct SingleSeedView<C> {
  lineages: Rc<SingleSeedLineages<C>>,
  prior_requests: HashMap<C, usize>,
}

impl<C> Default for SingleSeedView<C> {
  fn default() -> Self {
    SingleSeedView {
      lineages: Rc::new(RefCell::new(HashMap::new())),
      prior_requests: HashMap::new(),
    }
  }
}

#[derive(Debug)]
struct SingleSeedLineage {
  generated_values: Vec<f64>,
  generator: Pcg64Mcg,
}

type SingleSeedLineages<C> = RefCell<HashMap<C, SingleSeedLineage>>;

impl<G: GameState, C: Clone + Eq + Hash + ChoiceLineageIdentity<G>> SeedView<G>
  for SingleSeedView<C>
{
  type ChoiceLineageIdentity = C;

  fn gen(&mut self, identity: C) -> f64 {
    let mut lineages = self.lineages.borrow_mut();
    let prior_requests = self.prior_requests.entry(identity.clone()).or_insert(0);
    let lineage = lineages
      .entry(identity)
      .or_insert_with(|| SingleSeedLineage {
        generated_values: Vec::new(),
        generator: Pcg64Mcg::from_entropy(),
      });
    let result = lineage
      .generated_values
      .get(*prior_requests)
      .copied()
      .unwrap_or_else(|| {
        assert_eq!(*prior_requests, lineage.generated_values.len());
        let result = lineage.generator.gen();
        lineage.generated_values.push(result);
        result
      });
    *prior_requests += 1;
    result
  }
}

pub trait MaybeSeedView<G: GameState> {
  type SelfAsSeed: SeedView<G>;
  fn is_seed(&self) -> bool;
  fn as_seed(&mut self) -> Option<&mut Self::SelfAsSeed>;
}

impl<G: GameState, S: SeedView<G>> MaybeSeedView<G> for S {
  type SelfAsSeed = Self;
  fn is_seed(&self) -> bool {
    true
  }
  fn as_seed(&mut self) -> Option<&mut Self::SelfAsSeed> {
    Some(self)
  }
}

#[derive(Clone, Debug)]
pub struct NoRandomness;
#[derive(Clone, Debug)]
pub enum NeverSeed {}

impl<G: GameState> SeedView<G> for NeverSeed {
  type ChoiceLineageIdentity = ();

  fn gen(&mut self, _identity: Self::ChoiceLineageIdentity) -> f64 {
    unreachable!()
  }
}
