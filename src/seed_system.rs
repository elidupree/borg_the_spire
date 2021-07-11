use derivative::Derivative;
use ordered_float::NotNan;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use rand_pcg::Pcg64Mcg;
use serde::{Deserialize, Serialize};
use smallvec::{smallvec, SmallVec};
use std::cell::RefCell;
use std::fmt::Debug;
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

pub trait ChoiceLineages<G: GameState> {
  type Lineage;
  fn get_mut(
    &mut self,
    state: &G,
    fork_type: &G::RandomForkType,
    choice: &G::RandomChoice,
  ) -> &mut Self::Lineage;
}
pub trait ContainerKind {
  type Container<T: Clone + Debug + Default>: Clone + Debug + Default;
}

pub trait SeedView<G: GameState>: Debug {
  fn gen(&mut self, state: &G, fork_type: &G::RandomForkType, choice: &G::RandomChoice) -> f64;
}
pub trait Seed<G: GameState>: Clone + Debug {
  type View: SeedView<G>;
  fn view(&self) -> Self::View;
}
pub trait SeedGenerator<S>: Debug {
  fn make_seed(&mut self) -> S;
}

/// The presence or absence of a non-chosen choice has no effect on which choice is chosen,
/// and the presence or absence of ANY choice has no effect on how many times `seed.gen()` is called for any other choice.
pub fn choose_choice<G: GameState, S: SeedView<G> + ?Sized>(
  state: &G,
  fork_type: &G::RandomForkType,
  distribution: &Distribution<G::RandomChoice>,
  seed: &mut S,
) -> G::RandomChoice {
  distribution
    .0
    .iter()
    .min_by_key(|&(weight, choice)| {
      let value = seed.gen(state, fork_type, choice);
      NotNan::new(value / weight).unwrap()
    })
    .unwrap()
    .1
    .clone()
}

#[derive(Clone, Debug, Default)]
pub struct TrivialChoiceLineages<T>(T);
impl<G: GameState, T> ChoiceLineages<G> for TrivialChoiceLineages<T> {
  type Lineage = T;
  fn get_mut(
    &mut self,
    _state: &G,
    _fork_type: &G::RandomForkType,
    _choice: &G::RandomChoice,
  ) -> &mut T {
    &mut self.0
  }
}
pub struct TrivialChoiceLineagesKind;
impl ContainerKind for TrivialChoiceLineagesKind {
  type Container<T: Clone + Debug + Default> = TrivialChoiceLineages<T>;
}

#[derive(Clone, Debug)]
pub struct TrivialSeed {
  rng: Pcg64Mcg,
}
#[derive(Debug)]
pub struct TrivialSeedGenerator {
  source_rng: ChaCha8Rng,
}

impl TrivialSeed {
  pub fn new(rng: Pcg64Mcg) -> Self {
    TrivialSeed { rng }
  }
}
impl TrivialSeedGenerator {
  pub fn new(source_rng: ChaCha8Rng) -> Self {
    TrivialSeedGenerator { source_rng }
  }
}

impl<G: GameState> SeedView<G> for TrivialSeed {
  fn gen(&mut self, _state: &G, _fork_type: &G::RandomForkType, _choice: &G::RandomChoice) -> f64 {
    self.rng.gen()
  }
}
impl<G: GameState> Seed<G> for TrivialSeed {
  type View = TrivialSeed;

  fn view(&self) -> Self::View {
    self.clone()
  }
}
impl SeedGenerator<TrivialSeed> for TrivialSeedGenerator {
  fn make_seed(&mut self) -> TrivialSeed {
    self.make_seed()
  }
}
impl TrivialSeedGenerator {
  pub fn make_seed(&mut self) -> TrivialSeed {
    TrivialSeed::new(Pcg64Mcg::from_rng(&mut self.source_rng).unwrap())
  }
}

#[derive(Derivative)]
#[derivative(Debug(bound = ""))]
pub struct SingleSeedView<L: ContainerKind> {
  inner: Rc<RefCell<SingleSeedInner<L>>>,
  prior_requests: L::Container<u32>,
}
#[derive(Derivative)]
#[derivative(Debug(bound = ""))]
pub struct SingleSeedInner<L: ContainerKind> {
  source_rng: Pcg64Mcg,
  lineages: L::Container<SingleSeedLineage>,
}
#[derive(Derivative)]
#[derivative(Clone(bound = ""), Debug(bound = ""))]
pub struct SingleSeed<L: ContainerKind> {
  inner: Rc<RefCell<SingleSeedInner<L>>>,
}
#[derive(Debug)]
pub struct SingleSeedGenerator {
  source_rng: ChaCha8Rng,
}

impl<L: ContainerKind> SingleSeedView<L> {
  pub fn new(source_rng: Pcg64Mcg) -> Self {
    SingleSeedView {
      inner: Rc::new(RefCell::new(SingleSeedInner::new(source_rng))),
      prior_requests: Default::default(),
    }
  }
}

impl<L: ContainerKind> SingleSeedInner<L> {
  pub fn new(source_rng: Pcg64Mcg) -> Self {
    SingleSeedInner {
      source_rng,
      lineages: Default::default(),
    }
  }
}
impl<L: ContainerKind> SingleSeed<L> {
  pub fn new(source_rng: Pcg64Mcg) -> Self {
    SingleSeed {
      inner: Rc::new(RefCell::new(SingleSeedInner::new(source_rng))),
    }
  }
}
impl SingleSeedGenerator {
  pub fn new(source_rng: ChaCha8Rng) -> Self {
    SingleSeedGenerator { source_rng }
  }
}

#[derive(Clone, Debug, Default)]
pub struct SingleSeedLineage {
  generated_values: Vec<f64>,
}

impl<G: GameState, L: ContainerKind> SeedView<G> for SingleSeedView<L>
where
  L::Container<u32>: ChoiceLineages<G, Lineage = u32>,
  L::Container<SingleSeedLineage>: ChoiceLineages<G, Lineage = SingleSeedLineage>,
{
  fn gen(&mut self, state: &G, fork_type: &G::RandomForkType, choice: &G::RandomChoice) -> f64 {
    let mut inner_guard = self.inner.borrow_mut();
    let inner = &mut *inner_guard;
    let source_rng = &mut inner.source_rng;
    let prior_requests = self.prior_requests.get_mut(state, fork_type, choice);
    let lineage = inner.lineages.get_mut(state, fork_type, choice);
    let result = lineage
      .generated_values
      .get((*prior_requests) as usize)
      .copied()
      .unwrap_or_else(|| {
        assert_eq!((*prior_requests) as usize, lineage.generated_values.len());
        let result = source_rng.gen();
        lineage.generated_values.push(result);
        result
      });
    *prior_requests += 1;
    result
  }
}
impl<G: GameState, L: ContainerKind> Seed<G> for SingleSeed<L>
where
  L::Container<u32>: ChoiceLineages<G, Lineage = u32>,
  L::Container<SingleSeedLineage>: ChoiceLineages<G, Lineage = SingleSeedLineage>,
{
  type View = SingleSeedView<L>;
  fn view(&self) -> Self::View {
    SingleSeedView {
      inner: self.inner.clone(),
      prior_requests: Default::default(),
    }
  }
}
impl<L: ContainerKind> SeedGenerator<SingleSeed<L>> for SingleSeedGenerator {
  fn make_seed(&mut self) -> SingleSeed<L> {
    self.make_seed::<L>()
  }
}
impl SingleSeedGenerator {
  pub fn make_seed<L: ContainerKind>(&mut self) -> SingleSeed<L> {
    SingleSeed::new(Pcg64Mcg::from_rng(&mut self.source_rng).unwrap())
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
  fn gen(&mut self, _state: &G, _fork_type: &G::RandomForkType, _choice: &G::RandomChoice) -> f64 {
    unreachable!()
  }
}
