use crate::ai_utils::playout_result;
use crate::competing_optimizers::StrategyOptimizer;
use crate::condition_strategy::{
  Condition, ConditionStrategy, EvaluatedPriorities, EvaluationData, Rule,
};
use crate::representative_sampling::NewFractalRepresentativeSeedSearch;
use crate::seed_system::{Seed, SingleSeed, SingleSeedGenerator};
use crate::seeds_concrete::CombatChoiceLineagesKind;
use crate::simulation::{Runner, StandardRunner};
use crate::simulation_state::CombatState;
use rand::seq::SliceRandom;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use rand_distr::StandardNormal;
use serde::{Deserialize, Serialize};
use smallvec::alloc::fmt::Formatter;
use std::fmt;
use std::fmt::Display;
use std::ops::Deref;
use std::sync::Arc;
use std::time::{Duration, Instant};

pub type SeedSearch = NewFractalRepresentativeSeedSearch<
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
  FromSeedSearch,
}
#[derive(Copy, Clone, Serialize, Deserialize, Debug)]
pub enum HillClimbKind {
  BunchOfRandomChanges,
  BunchOfRandomChangesInspired,
  OneRelevantRule,
}
impl StrategyGeneratorsWithSharedRepresenativeSeeds {
  pub fn new(
    starting_state: CombatState,
    rng: &mut impl Rng,
  ) -> StrategyGeneratorsWithSharedRepresenativeSeeds {
    let mut generators = Vec::new();
    for steps in (0..=8).map(|i| 1 << i) {
      for num_verification_seeds in (0..=5).map(|i| 1 << i) {
        for &start in &[HillClimbStart::NewRandom, HillClimbStart::FromSeedSearch] {
          for &kind in &[
            HillClimbKind::BunchOfRandomChanges,
            HillClimbKind::BunchOfRandomChangesInspired,
            HillClimbKind::OneRelevantRule,
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
      seed_search: NewFractalRepresentativeSeedSearch::new(
        starting_state,
        SingleSeedGenerator::new(ChaCha8Rng::from_rng(rng).unwrap()),
        Default::default(),
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
    self.seed_search.consider_strategy(
      Arc::new(strategy),
      generator.generator.min_playouts_before_culling(),
      rng,
    );
  }
}

impl GeneratorKind {
  pub fn min_playouts_before_culling(&self) -> usize {
    match self {
      &GeneratorKind::HillClimb { steps, .. } => steps.min(32),
    }
  }
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
          HillClimbStart::FromSeedSearch => seed_search
            .strategies
            .choose(rng)
            .unwrap()
            .strategy
            .deref()
            .clone(),
        };
        let mut verification_seeds: Vec<_> = seed_search
          .seeds
          .iter()
          .take(num_verification_seeds)
          .collect();

        // hack - the seed search may not have generated this many (or any) seeds yet
        let extra_seeds;
        if verification_seeds.len() < num_verification_seeds {
          extra_seeds = (verification_seeds.len()..num_verification_seeds)
            .map(|_| SingleSeed::new(rng))
            .collect::<Vec<_>>();
          verification_seeds.extend(extra_seeds.iter());
        }
        let mut verification_seeds: Vec<_> = verification_seeds
          .into_iter()
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
                .strategies
                .iter()
                .map(|s| &*s.strategy)
                .collect::<Vec<_>>(),
            ),
            HillClimbKind::OneRelevantRule => {
              let mut state = seed_search.starting_state.clone();
              let mut runner = StandardRunner::new(&mut state, first.seed.view());
              let mut candidate_rules = Vec::new();
              while !runner.state().combat_over() {
                let state = runner.state();
                let data = EvaluationData::new(state);
                let priorities = EvaluatedPriorities::evaluated(&current.rules, state, &data);
                let best_index = priorities.best_index();
                for _ in 0..50 {
                  let condition =
                    Condition::random_generally_relevant_choice_distinguisher(state, rng);
                  let mut rule = Rule {
                    conditions: vec![condition],
                    flat_reward: rng.sample(StandardNormal),
                    ..Default::default()
                  };
                  if priorities.best_index_with_extra_rule(&rule, state, &data) != best_index {
                    for _ in 0..rng.gen_range(0..=2) {
                      for _ in 0..50 {
                        let condition =
                          Condition::random_generally_relevant_state_distinguisher(state, rng);
                        if condition.evaluate(state, &data.contexts().next().unwrap()) {
                          rule.conditions.push(condition);
                          break;
                        }
                      }
                    }
                    candidate_rules.push(rule);
                    break;
                  }
                }
                let choice = &data.choices[best_index].choice;
                runner.apply_choice(&choice);
              }
              let mut new = current.clone();
              if let Some(new_rule) = candidate_rules.choose(rng) {
                new.rules.push(new_rule.clone())
              }
              new
            }
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
    let result = self.seed_search.best_strategy();
    self.seed_search.report();
    println!("StrategyGeneratorsWithSharedRepresenativeSeeds top strategies:");
    for strategy in &self.seed_search.strategies {
      println!("{}", strategy.strategy.annotation);
    }
    result
  }
}
