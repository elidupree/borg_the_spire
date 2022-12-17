use derivative::Derivative;
use ordered_float::NotNan;
use parking_lot::{Mutex, MutexGuard};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use rand_pcg::Pcg64Mcg;
use seahash::SeaHasher;
use serde::{Deserialize, Serialize};
use smallvec::{smallvec, SmallVec};
use std::fmt::Debug;
use std::hash::{Hash, Hasher};
use std::ops::{Add, AddAssign, Mul};

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
  type RandomForkType: Hash + Debug;
  type RandomChoice: Clone + Hash + Debug;
}

pub trait ChoiceLineageIdentity<G: GameState>: Copy + Clone + Hash {
  fn lineage_identity(state: &G, fork_type: &G::RandomForkType, choice: &G::RandomChoice) -> Self;
}
pub trait ChoiceLineages: Clone + Debug + Default + Send + Sync {
  type LineageIdentity;
  type Lineage: Debug;
  fn get_mut(&mut self, identity: Self::LineageIdentity) -> &mut Self::Lineage;
}
pub trait ChoiceLineagesKind {
  type LineageIdentity: Debug;
  type Lineages<T: Clone + Debug + Default + Send + Sync>: ChoiceLineages<
    LineageIdentity = Self::LineageIdentity,
    Lineage = T,
  >;
}

pub trait SeedView<G: GameState>: Clone + Debug {
  fn gen(&mut self, state: &G, fork_type: &G::RandomForkType, choice: &G::RandomChoice) -> f64;
}
pub trait Seed<G: GameState>: Clone + Debug + Send + Sync {
  type View<'a>: SeedView<G>
  where
    Self: 'a;
  fn view(&self) -> Self::View<'_>;
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
pub struct TrivialChoiceLineagesKind;
impl<G: GameState> ChoiceLineageIdentity<G> for () {
  fn lineage_identity(
    _state: &G,
    _fork_type: &G::RandomForkType,
    _choice: &G::RandomChoice,
  ) -> Self {
    ()
  }
}
impl<T: Clone + Debug + Default + Send + Sync> ChoiceLineages for TrivialChoiceLineages<T> {
  type LineageIdentity = ();
  type Lineage = T;
  fn get_mut(&mut self, _identity: ()) -> &mut T {
    &mut self.0
  }
}
impl ChoiceLineagesKind for TrivialChoiceLineagesKind {
  type LineageIdentity = ();
  type Lineages<T: Clone + Debug + Default + Send + Sync> = TrivialChoiceLineages<T>;
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
  type View<'a> = TrivialSeed;

  fn view(&self) -> Self::View<'_> {
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
pub struct SingleSeedView<'a, L: ChoiceLineagesKind> {
  seed: &'a SingleSeed<L>,
  prior_requests: L::Lineages<u32>,
  caches: MutexGuard<'a, L::Lineages<SingleSeedLineage>>,
}
// neither Rust nor Derivative can currently derive Clone for the above, sigh
impl<'a, L: ChoiceLineagesKind> Clone for SingleSeedView<'a, L> {
  fn clone(&self) -> Self {
    SingleSeedView {
      seed: self.seed,
      prior_requests: self.prior_requests.clone(),
      caches: panic!(),
    }
  }
}
#[derive(Derivative)]
#[derivative(Debug(bound = ""))]
pub struct SingleSeed<L: ChoiceLineagesKind> {
  hasher_seeds: [u64; 4],
  caches: Mutex<L::Lineages<SingleSeedLineage>>,
}
impl<'a, L: ChoiceLineagesKind> Clone for SingleSeed<L> {
  fn clone(&self) -> Self {
    SingleSeed {
      hasher_seeds: self.hasher_seeds,
      caches: Default::default(),
    }
  }
}
#[derive(Debug)]
pub struct SingleSeedGenerator {
  source_rng: ChaCha8Rng,
}

impl<L: ChoiceLineagesKind> SingleSeed<L> {
  pub fn new(source_rng: &mut impl Rng) -> Self {
    SingleSeed {
      hasher_seeds: source_rng.gen(),
      caches: Default::default(),
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
  generated_values: SmallVec<[f64; 2]>,
}

impl<'a, G: GameState, L: ChoiceLineagesKind> SeedView<G> for SingleSeedView<'a, L>
where
  L::LineageIdentity: ChoiceLineageIdentity<G>,
{
  fn gen(&mut self, state: &G, fork_type: &G::RandomForkType, choice: &G::RandomChoice) -> f64 {
    let identity = L::LineageIdentity::lineage_identity(state, fork_type, choice);
    let hasher_seeds = &self.seed.hasher_seeds;
    let prior_requests = self.prior_requests.get_mut(identity);
    let cache = self.caches.get_mut(identity);
    // if *prior_requests > 3 {
    //   dbg!((*prior_requests, fork_type, choice, identity));
    // }
    let result = cache
      .generated_values
      .get((*prior_requests) as usize)
      .copied()
      .unwrap_or_else(|| {
        //assert_eq!((*prior_requests) as usize, lineage.generated_values.len());
        let [k1, k2, k3, k4] = *hasher_seeds;
        let mut hasher = SeaHasher::with_seeds(k1, k2, k3, k4);
        prior_requests.hash(&mut hasher);
        identity.hash(&mut hasher);
        let result = hasher.finish() as f64;
        cache.generated_values.push(result);
        result
      });
    *prior_requests += 1;
    result
  }
}
impl<G: GameState, L: ChoiceLineagesKind + 'static> Seed<G> for SingleSeed<L>
where
  L::LineageIdentity: ChoiceLineageIdentity<G>,
{
  type View<'a> = SingleSeedView<'a, L>;
  fn view(&self) -> Self::View<'_> {
    SingleSeedView {
      seed: self,
      prior_requests: Default::default(),
      caches: self.caches.lock(),
    }
  }
}
impl<L: ChoiceLineagesKind> SeedGenerator<SingleSeed<L>> for SingleSeedGenerator {
  fn make_seed(&mut self) -> SingleSeed<L> {
    self.make_seed::<L>()
  }
}
impl SingleSeedGenerator {
  pub fn make_seed<L: ChoiceLineagesKind>(&mut self) -> SingleSeed<L> {
    SingleSeed::new(&mut self.source_rng)
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
