use crate::ai_utils::playout_result;
use crate::competing_optimizers::StrategyOptimizer;
use crate::condition_strategy::ConditionStrategy;
use crate::representative_sampling::FractalRepresentativeSeedSearch;
use crate::seed_system::{Seed, SingleSeed, SingleSeedGenerator};
use crate::seeds_concrete::CombatChoiceLineagesKind;
use crate::simulation_state::CombatState;
use rand::seq::{IteratorRandom, SliceRandom};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};
use smallvec::alloc::fmt::Formatter;
use std::fmt;
use std::fmt::Display;
use std::ops::Deref;
use std::sync::Arc;
use std::time::{Duration, Instant};

pub type SeedSearch = FractalRepresentativeSeedSearch<
  ConditionStrategy,
  SingleSeed<CombatChoiceLineagesKind>,
  SingleSeedGenerator,
>;

pub struct StrategyGeneratorsWithSharedRepresenativeSeeds {
  pub seed_search: SeedSearch,
  pub generators: Vec<SharingGenerator>,
}

pub struct SharingGenerator {
  pub time_used: Duration,
  pub generator: GeneratorKind,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum GeneratorKind {
  HillClimb {
    steps: usize,
    num_verification_seeds: usize,
    start: HillClimbStart,
    kind: HillClimbKind,
  },
}

#[derive(Copy, Clone, Serialize, Deserialize, Debug)]
pub enum HillClimbStart {
  NewRandom,
  FromBiggestLayer,
}
#[derive(Copy, Clone, Serialize, Deserialize, Debug)]
pub enum HillClimbKind {
  BunchOfRandomChanges,
  BunchOfRandomChangesInspired,
}
impl StrategyGeneratorsWithSharedRepresenativeSeeds {
  pub fn new(
    starting_state: CombatState,
    rng: &mut impl Rng,
  ) -> StrategyGeneratorsWithSharedRepresenativeSeeds {
    let strategy = Arc::new(ConditionStrategy::fresh_distinctive_candidate(
      &starting_state,
      rng,
    ));
    let mut generators = Vec::new();
    for steps in (0..=10).map(|i| 1 << i) {
      for num_verification_seeds in (0..=5).map(|i| 1 << i) {
        for &start in &[HillClimbStart::NewRandom, HillClimbStart::FromBiggestLayer] {
          for &kind in &[
            HillClimbKind::BunchOfRandomChanges,
            HillClimbKind::BunchOfRandomChangesInspired,
          ] {
            generators.push(SharingGenerator {
              time_used: Duration::from_secs(0),
              generator: GeneratorKind::HillClimb {
                steps,
                num_verification_seeds,
                start,
                kind,
              },
            });
          }
        }
      }
    }
    StrategyGeneratorsWithSharedRepresenativeSeeds {
      seed_search: FractalRepresentativeSeedSearch::new(
        starting_state,
        SingleSeedGenerator::new(ChaCha8Rng::from_rng(rng).unwrap()),
        strategy,
      ),
      generators,
    }
  }

  pub fn step(&mut self, rng: &mut impl Rng) {
    let generator = self
      .generators
      .iter_mut()
      .min_by_key(|g| g.time_used)
      .unwrap();
    let start = Instant::now();
    let strategy = generator.generator.gen_strategy(&self.seed_search, rng);
    let duration = start.elapsed();
    generator.time_used += duration;
    self.seed_search.consider_strategy(Arc::new(strategy));
  }
}

impl GeneratorKind {
  pub fn gen_strategy(&self, seed_search: &SeedSearch, rng: &mut impl Rng) -> ConditionStrategy {
    match self {
      &GeneratorKind::HillClimb {
        steps,
        num_verification_seeds,
        start,
        kind,
      } => {
        struct SeedInfo<'a> {
          seed: &'a SingleSeed<CombatChoiceLineagesKind>,
          current_score: f64,
        }
        let mut current = match start {
          HillClimbStart::NewRandom => {
            ConditionStrategy::fresh_distinctive_candidate(&seed_search.starting_state, rng)
          }
          HillClimbStart::FromBiggestLayer => seed_search
            .layers
            .last()
            .unwrap()
            .strategies()
            .choose(rng)
            .unwrap()
            .strategy
            .deref()
            .clone(),
        };
        let mut seeds_source: Vec<_> = std::iter::once(&seed_search.lowest_seeds)
          .chain(seed_search.layers.iter().map(|l| &l.seeds))
          .find(|s| s.len() >= num_verification_seeds)
          .unwrap_or_else(|| &seed_search.layers.last().unwrap().seeds)
          .iter()
          .collect();
        seeds_source.shuffle(rng);
        let mut verification_seeds: Vec<_> = seeds_source
          .into_iter()
          .take(num_verification_seeds)
          .map(|s| SeedInfo {
            seed: s,
            current_score: playout_result(&seed_search.starting_state, s.view(), &current).score,
          })
          .collect();
        for _ in 0..steps {
          verification_seeds.shuffle(rng);
          let (first, rest) = verification_seeds.split_first().unwrap();
          let new = match kind {
            HillClimbKind::BunchOfRandomChanges => {
              current.hill_climb_candidate(&seed_search.starting_state, rng, &[])
            }
            HillClimbKind::BunchOfRandomChangesInspired => current.hill_climb_candidate(
              &seed_search.starting_state,
              rng,
              &seed_search
                .layers
                .last()
                .unwrap()
                .strategies()
                .map(|s| &*s.strategy)
                .collect::<Vec<_>>(),
            ),
          };
          let first_score =
            playout_result(&seed_search.starting_state, first.seed.view(), &new).score;
          if first_score <= verification_seeds[0].current_score {
            continue;
          }
          let new_scores: Vec<_> = std::iter::once(first_score)
            .chain(
              rest
                .iter()
                .map(|s| playout_result(&seed_search.starting_state, s.seed.view(), &new).score),
            )
            .collect();
          if new_scores.iter().sum::<f64>()
            > verification_seeds
              .iter()
              .map(|s| s.current_score)
              .sum::<f64>()
          {
            current = new;
            for (info, new_score) in verification_seeds.iter_mut().zip(new_scores) {
              info.current_score = new_score;
            }
          }
        }

        current.annotation = format!("{} + {}", current.annotation, self);

        current
      }
    }
  }
}

impl Display for GeneratorKind {
  fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
    match self {
      GeneratorKind::HillClimb {
        steps,
        num_verification_seeds,
        start: _,
        kind,
      } => {
        write!(f, "{}x{:?}@{}", steps, kind, num_verification_seeds)
      }
    }
  }
}

impl StrategyOptimizer for StrategyGeneratorsWithSharedRepresenativeSeeds {
  type Strategy = ConditionStrategy;
  fn step(&mut self, _state: &CombatState, rng: &mut ChaCha8Rng) {
    self.step(rng);
  }

  fn report(&self) -> Arc<Self::Strategy> {
    let result = self
      .seed_search
      .layers
      .last()
      .unwrap()
      .best_strategy
      .strategy
      .clone();
    println!(
      "StrategyGeneratorsWithSharedRepresenativeSeeds reporting strategy generated by: {}",
      result.annotation
    );
    result
  }
}
