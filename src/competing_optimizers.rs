use ordered_float::{NotNan, OrderedFloat};
use rand::seq::SliceRandom;
use rand_pcg::Pcg64Mcg;
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

//use crate::actions::*;
use crate::ai_utils;
use crate::ai_utils::{collect_starting_points, playout_narration, Strategy};
use crate::condition_strategy::ConditionStrategy;
use crate::condition_strategy_generators::StrategyGeneratorsWithSharedRepresenativeSeeds;
use crate::neural_net_ai::NeuralStrategy;
use crate::representative_sampling::{
  FractalRepresentativeSeedSearchExplorationOptimizerKind,
  NewFractalRepresentativeSeedSearchExplorationOptimizerKind,
};
use crate::seed_system::{Seed, SingleSeed, SingleSeedGenerator, TrivialSeed};
use crate::seeds_concrete::CombatChoiceLineagesKind;
use crate::simulation::*;
use crate::simulation_state::*;
use crate::start_and_strategy_ai::FastStrategy;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use std::collections::BTreeMap;
use std::sync::Arc;

#[allow(unused)]
pub trait StrategyOptimizer: 'static {
  type Strategy: Strategy;
  fn step(&mut self, state: &CombatState, rng: &mut ChaCha8Rng);
  fn report(&self) -> Arc<Self::Strategy>;
  fn print_extra_info(&self, state: &CombatState) {}
}

pub trait ExplorationOptimizerKind {
  type ExplorationOptimizer<T: Strategy + 'static>: StrategyOptimizer;
  fn new<T: Strategy + 'static>(
    self,
    starting_state: &CombatState,
    rng: &mut ChaCha8Rng,
    new_strategy: Box<dyn Fn(&[&T]) -> T>,
  ) -> Self::ExplorationOptimizer<T>;
}

pub struct CandidateStrategy<T> {
  strategy: Arc<T>,
  playouts: usize,
  total_score: f64,
}

// Note: This meta strategy often performed WORSE than the naive strategy it's based on,
// probably because it chose lucky moves rather than good moves
#[derive(Clone, Debug)]
struct MetaStrategy<'a, T>(&'a T);

impl<'a, T: Strategy> Strategy for MetaStrategy<'a, T> {
  fn choose_choice(&self, state: &CombatState) -> Vec<Choice> {
    let combos = collect_starting_points(state.clone(), 200);
    let choices = combos.into_iter().map(|(mut state, choices)| {
      StandardRunner::new(&mut state, TrivialSeed::new(Pcg64Mcg::from_entropy()))
        .run_until_unable();
      let num_attempts = 200;
      let score = (0..num_attempts)
        .map(|_| {
          ai_utils::playout_result(&state, TrivialSeed::new(Pcg64Mcg::from_entropy()), self.0).score
        })
        .sum::<f64>()
        / num_attempts as f64;
      (choices, score)
    });
    choices
      .max_by_key(|(_, score)| OrderedFloat(*score))
      .unwrap()
      .0
  }
}

pub struct OriginalExplorationOptimizerKind;
pub struct OriginalExplorationOptimizer<T> {
  candidate_strategies: Vec<CandidateStrategy<T>>,
  new_strategy: Box<dyn Fn(&[&T]) -> T>,
  passes: usize,
  current_pass_index: usize,
}

impl<T> OriginalExplorationOptimizer<T> {
  pub fn max_strategy_playouts(&self) -> usize {
    ((self.passes as f64).sqrt() + 2.0) as usize
  }

  pub fn new(new_strategy: Box<dyn Fn(&[&T]) -> T>) -> Self {
    OriginalExplorationOptimizer {
      candidate_strategies: Vec::new(),
      new_strategy,
      passes: 0,
      current_pass_index: 0,
    }
  }

  fn best_strategy(&self) -> &CandidateStrategy<T> {
    // not the best average score, but the most-explored, which comes out to best average score at last sorting among strategies that are at the max playouts
    // note that this function may be called in the middle of a pass, when the current best strategy has not yet been visited to increase its number of playouts to the new maximum, so don't rely on the given maximum;
    // since this function chooses the FIRST qualifying strategy, it's based on the most recent time the strategies were sorted, so the score-dependence of this choice isn't biased by the change in score variance from some of them having one extra playout.
    &self
      .candidate_strategies
      .iter()
      .enumerate()
      .max_by_key(|(index, strategy)| (strategy.playouts, -(*index as i32)))
      .unwrap()
      .1
  }
}

impl ExplorationOptimizerKind for OriginalExplorationOptimizerKind {
  type ExplorationOptimizer<T: Strategy + 'static> = OriginalExplorationOptimizer<T>;

  fn new<T: Strategy + 'static>(
    self,
    _starting_state: &CombatState,
    _rng: &mut ChaCha8Rng,
    new_strategy: Box<dyn Fn(&[&T]) -> T>,
  ) -> Self::ExplorationOptimizer<T> {
    OriginalExplorationOptimizer::new(new_strategy)
  }
}

impl<T: Strategy + 'static> StrategyOptimizer for OriginalExplorationOptimizer<T> {
  type Strategy = T;
  fn step(&mut self, state: &CombatState, _rng: &mut ChaCha8Rng) {
    loop {
      if self.current_pass_index >= self.candidate_strategies.len() {
        self
          .candidate_strategies
          .sort_by_key(|strategy| OrderedFloat(-strategy.total_score / strategy.playouts as f64));
        let mut index = 0;
        self.candidate_strategies.retain(|strategy| {
          index += 1;
          strategy.playouts >= index
        });

        self.passes += 1;
        self.candidate_strategies.push(CandidateStrategy {
          strategy: Arc::new((self.new_strategy)(
            &self
              .candidate_strategies
              .iter()
              .map(|c| &*c.strategy)
              .collect::<Vec<_>>(),
          )),
          playouts: 0,
          total_score: 0.0,
        });
        self.current_pass_index = 0;
      }

      let max_strategy_playouts = self.max_strategy_playouts();
      let strategy = &mut self.candidate_strategies[self.current_pass_index];
      self.current_pass_index += 1;

      if strategy.playouts < max_strategy_playouts {
        let result = ai_utils::playout_result(
          state,
          TrivialSeed::new(Pcg64Mcg::from_entropy()),
          &*strategy.strategy,
        );
        strategy.total_score += result.score;
        strategy.playouts += 1;
        return;
      }
    }
  }

  fn report(&self) -> Arc<Self::Strategy> {
    let best = self.best_strategy();

    println!(
      "OriginalExplorationOptimizer reporting strategy with {} playouts, running average {}",
      best.playouts,
      (best.total_score / best.playouts as f64)
    );

    best.strategy.clone()
  }
}

pub struct IndependentSeedsExplorationOptimizerKind {
  num_seeds: usize,
}
pub struct IndependentSeedsExplorationOptimizer<T> {
  candidate_strategies: BTreeMap<NotNan<f64>, Arc<T>>,
  new_strategy: Box<dyn Fn(&[&T]) -> T>,
  seeds: Vec<SingleSeed<CombatChoiceLineagesKind>>,
  steps: usize,
  total_accepted: usize,
}

impl<T> IndependentSeedsExplorationOptimizer<T> {
  pub fn new(
    rng: &mut ChaCha8Rng,
    num_seeds: usize,
    new_strategy: Box<dyn Fn(&[&T]) -> T>,
  ) -> Self {
    let mut generator = SingleSeedGenerator::new(ChaCha8Rng::from_rng(rng).unwrap());
    IndependentSeedsExplorationOptimizer {
      candidate_strategies: BTreeMap::new(),
      new_strategy,
      seeds: (0..num_seeds).map(|_| generator.make_seed()).collect(),
      steps: 0,
      total_accepted: 0,
    }
  }
}

impl ExplorationOptimizerKind for IndependentSeedsExplorationOptimizerKind {
  type ExplorationOptimizer<T: Strategy + 'static> = IndependentSeedsExplorationOptimizer<T>;

  fn new<T: Strategy + 'static>(
    self,
    _starting_state: &CombatState,
    rng: &mut ChaCha8Rng,
    new_strategy: Box<dyn Fn(&[&T]) -> T>,
  ) -> Self::ExplorationOptimizer<T> {
    IndependentSeedsExplorationOptimizer::new(rng, self.num_seeds, new_strategy)
  }
}

impl<T: Strategy + 'static> StrategyOptimizer for IndependentSeedsExplorationOptimizer<T> {
  type Strategy = T;
  fn step(&mut self, state: &CombatState, _rng: &mut ChaCha8Rng) {
    self.steps += 1;
    let target_count = 1 + self.steps.next_power_of_two().trailing_zeros() as usize;
    self.seeds.shuffle(&mut rand::thread_rng());
    let strategy = (self.new_strategy)(
      &self
        .candidate_strategies
        .values()
        .map(|s| &**s)
        .collect::<Vec<_>>(),
    );
    let mut total_score = 0.0;
    for (index, seed) in self.seeds.iter().enumerate() {
      let result = ai_utils::playout_result(state, seed.view(), &strategy);
      total_score += result.score;
      let average = total_score / (index + 1) as f64;
      if target_count <= self.candidate_strategies.len()
        && average
          < self
            .candidate_strategies
            .first_key_value()
            .unwrap()
            .0
            .into_inner()
      {
        return;
      }
    }
    let average = total_score / self.seeds.len() as f64;
    self
      .candidate_strategies
      .insert(NotNan::new(average).unwrap(), Arc::new(strategy));
    self.total_accepted += 1;
    if self.candidate_strategies.len() > target_count {
      self.candidate_strategies.pop_first();
    }
  }

  fn report(&self) -> Arc<Self::Strategy> {
    let (average, best) = self.candidate_strategies.last_key_value().unwrap();

    println!(
      "IndependentSeedsExplorationOptimizer reporting strategy with average score of {} (worst: {}, count: {}, total accepted: {}, steps: {})",
      average, self.candidate_strategies.first_key_value().unwrap().0, self.candidate_strategies.len(), self.total_accepted, self.steps
    );

    best.clone()
  }
}

impl StrategyOptimizer for NeuralStrategy {
  type Strategy = NeuralStrategy;
  fn step(&mut self, state: &CombatState, _rng: &mut ChaCha8Rng) {
    self.do_training_playout(state);
  }

  fn report(&self) -> Arc<Self::Strategy> {
    Arc::new(self.clone())
  }
}

pub fn optimizer_step(
  name: &str,
  state: &CombatState,
  rng: &mut ChaCha8Rng,
  optimizer: &mut impl StrategyOptimizer,
  last: bool,
) {
  println!("Optimizing {}…", name);
  let start = Instant::now();
  let mut steps = 0;
  let elapsed = loop {
    optimizer.step(state, rng);
    steps += 1;
    let elapsed = start.elapsed();
    if elapsed > Duration::from_millis(2000) {
      break elapsed;
    }
  };

  println!(
    "Optimized {} for {:.2?} ({} steps). Reporting…",
    name, elapsed, steps
  );
  let strategy = optimizer.report();

  let start = Instant::now();
  let mut steps = 0;
  let mut total_test_score = 0.0;
  let test_duration = Duration::from_millis(if last { 10000 } else { 500 });
  let elapsed = loop {
    total_test_score += ai_utils::playout_result(
      state,
      TrivialSeed::new(Pcg64Mcg::from_entropy()),
      &*strategy,
    )
    .score;
    steps += 1;

    let elapsed = start.elapsed();
    if elapsed > test_duration {
      break elapsed;
    }
  };

  println!(
    "Evaluated {} for {:.2?} ({} playouts). Average score: {}",
    name,
    elapsed,
    steps,
    total_test_score / steps as f64
  );

  if last {
    println!(
      "Example playout:\n{}",
      playout_narration(
        state,
        TrivialSeed::new(Pcg64Mcg::from_entropy()),
        &*strategy
      )
    );
    optimizer.print_extra_info(state);
  }

  /*let start = Instant::now();
  let mut steps = 0;
  let mut total_test_score = 0.0;
  let elapsed = loop {
    total_test_score += playout_result(state, &MetaStrategy(strategy)).score;
    steps += 1;

    let elapsed = start.elapsed();
    if elapsed > Duration::from_millis(5000*20) {
      break elapsed;
    }
  };

  println!( "Evaluated meta-strategy for {} for {:.2?} ({} playouts). Average score: {}", name, elapsed, steps, total_test_score / steps as f64) ;*/
}

/*
pub fn run_benchmark (name: & str, state: & CombatState, optimization_playouts: usize, test_playouts: usize, mut optimizer: impl StrategyOptimizer) {
  println!( "Starting benchmark for {}, doing {} optimization playouts…", name, optimization_playouts);

  for iteration in 0..optimization_playouts {
    optimizer.step (| strategy | {
      let mut state = state.clone();
      play_out (
        &mut Runner::new (&mut state, true, false),
        strategy,
      );
      CombatResult::new (& state)
    });

    if iteration % 10000 == 9999 {
      println!( "Completed {} playouts…", iteration + 1);
    }
  }

  let (best_strategy, anticipated_score) = optimizer.current_best();

  println!( "Optimization completed for {}. Found strategy with anticipated score {}. Doing {} test playouts…", name, anticipated_score, test_playouts);

  let total_test_score: f64 = (0..test_playouts)
    .map(|_| {
      let mut state = state.clone();
      play_out (
        &mut Runner::new (&mut state, true, false),
        best_strategy,
      );
      CombatResult::new (& state).score
    })
    .sum();

  println!( "Testing completed for {}. Final average score: {}.", name, total_test_score/test_playouts as f64);
  println!();
}*/

pub trait Competitor {
  fn step(&mut self, state: &CombatState, rng: &mut ChaCha8Rng, last: bool);
}
struct OptimizerCompetitor<T> {
  name: String,
  optimizer: T,
}
impl<T: StrategyOptimizer> Competitor for OptimizerCompetitor<T> {
  fn step(&mut self, state: &CombatState, rng: &mut ChaCha8Rng, last: bool) {
    optimizer_step(&self.name, state, rng, &mut self.optimizer, last);
  }
}

#[derive(Serialize, Deserialize, Debug)]
pub enum CompetitorSpecification {
  ExplorationOptimizer(
    ExplorationOptimizerKindSpecification,
    StrategyAndGeneratorSpecification,
  ),
  ConditionStrategyGenerators,
}
#[derive(Copy, Clone, Serialize, Deserialize, Debug)]
pub enum StrategyAndGeneratorSpecification {
  FastRandom,
  FastGenetic,
  ConditionStrategy,
  NeuralRandom,
  NeuralMutating,
}
#[derive(Copy, Clone, Serialize, Deserialize, Debug)]
pub enum ExplorationOptimizerKindSpecification {
  Original,
  IndependentSeeds(usize),
  FractalRepresentativeSeedSearch,
  NewFractalRepresentativeSeedSearch(NewFractalRepresentativeSeedSearchExplorationOptimizerKind),
}

impl CompetitorSpecification {
  pub fn build(self, starting_state: &CombatState, rng: &mut ChaCha8Rng) -> Box<dyn Competitor> {
    match self {
      CompetitorSpecification::ExplorationOptimizer(optimizer, strategy) => {
        optimizer.build(strategy, starting_state, rng)
      }
      CompetitorSpecification::ConditionStrategyGenerators => Box::new(OptimizerCompetitor {
        name: "ConditionStrategyGenerators".to_string(),
        optimizer: StrategyGeneratorsWithSharedRepresenativeSeeds::new(starting_state.clone(), rng),
      }),
    }
  }
}
impl ExplorationOptimizerKindSpecification {
  pub fn build(
    self,
    strategy: StrategyAndGeneratorSpecification,
    starting_state: &CombatState,
    rng: &mut ChaCha8Rng,
  ) -> Box<dyn Competitor> {
    match self {
      ExplorationOptimizerKindSpecification::Original => {
        strategy.build(OriginalExplorationOptimizerKind, self, starting_state, rng)
      }
      ExplorationOptimizerKindSpecification::IndependentSeeds(num_seeds) => strategy.build(
        IndependentSeedsExplorationOptimizerKind { num_seeds },
        self,
        starting_state,
        rng,
      ),
      ExplorationOptimizerKindSpecification::FractalRepresentativeSeedSearch => strategy.build(
        FractalRepresentativeSeedSearchExplorationOptimizerKind,
        self,
        starting_state,
        rng,
      ),
      ExplorationOptimizerKindSpecification::NewFractalRepresentativeSeedSearch(kind) => {
        strategy.build(kind, self, starting_state, rng)
      }
    }
  }
}
impl StrategyAndGeneratorSpecification {
  pub fn build<K: ExplorationOptimizerKind>(
    self,
    kind: K,
    optimizer: ExplorationOptimizerKindSpecification,
    starting_state: &CombatState,
    rng: &mut ChaCha8Rng,
  ) -> Box<dyn Competitor> {
    let name = format!("{:?}/{:?}", optimizer, self);
    let starting_state_clone = starting_state.clone();
    match self {
      StrategyAndGeneratorSpecification::FastRandom => Box::new(OptimizerCompetitor {
        name,
        optimizer: kind.new(
          starting_state,
          rng,
          Box::new(|_: &[&FastStrategy]| FastStrategy::random(&mut rand::thread_rng())),
        ),
      }),
      StrategyAndGeneratorSpecification::FastGenetic => Box::new(OptimizerCompetitor {
        name,
        optimizer: kind.new(
          starting_state,
          rng,
          Box::new(|candidates: &[&FastStrategy]| {
            if candidates.len() < 2 {
              FastStrategy::random(&mut rand::thread_rng())
            } else {
              FastStrategy::offspring(
                &candidates
                  .choose_multiple(&mut rand::thread_rng(), 2)
                  .copied()
                  .collect::<Vec<_>>(),
                &mut rand::thread_rng(),
              )
            }
          }),
        ),
      }),
      StrategyAndGeneratorSpecification::ConditionStrategy => Box::new(OptimizerCompetitor {
        name,
        optimizer: kind.new(
          starting_state,
          rng,
          Box::new(move |candidates: &[&ConditionStrategy]| {
            if rand::random() || candidates.is_empty() {
              ConditionStrategy::fresh_distinctive_candidate(
                &starting_state_clone,
                &mut rand::thread_rng(),
              )
            } else {
              candidates
                .choose(&mut rand::thread_rng())
                .unwrap()
                .bunch_of_random_changes(&starting_state_clone, &mut rand::thread_rng(), candidates)
            }
          }),
        ),
      }),
      StrategyAndGeneratorSpecification::NeuralRandom => Box::new(OptimizerCompetitor {
        name,
        optimizer: kind.new(
          starting_state,
          rng,
          Box::new(|_: &[&NeuralStrategy]| NeuralStrategy::new_random(16)),
        ),
      }),
      StrategyAndGeneratorSpecification::NeuralMutating => Box::new(OptimizerCompetitor {
        name,
        optimizer: kind.new(
          starting_state,
          rng,
          Box::new(|candidates: &[&NeuralStrategy]| {
            if candidates.len() < 1 || rand::random::<f64>() < 0.4 {
              NeuralStrategy::new_random(16)
            } else {
              candidates
                .choose(&mut rand::thread_rng())
                .unwrap()
                .mutated()
            }
          }),
        ),
      }),
    }
  }
}

pub fn run(competitors: impl IntoIterator<Item = CompetitorSpecification>) {
  let ghost_file = std::fs::File::open("data/hexaghost.json").unwrap();
  let ghost_state: CombatState =
    serde_json::from_reader(std::io::BufReader::new(ghost_file)).unwrap();
  let mut rng = ChaCha8Rng::from_entropy();
  let mut competitors: Vec<_> = competitors
    .into_iter()
    .map(|s| CompetitorSpecification::build(s, &ghost_state, &mut rng))
    .collect();
  for iteration in 0..20 {
    println!("\nIteration {}:", iteration);
    for competitor in &mut competitors {
      competitor.step(&ghost_state, &mut rng, iteration == 19);
    }
    println!();
  }
  //let optimization_playouts = 1000000;
  //let test_playouts = 10000;
  //let mut neural_training_only = NeuralStrategy::new_random(&ghost_state, 16);

  /*let mut neural_random_training: ExplorationOptimizer<NeuralStrategy, _> =
  ExplorationOptimizer::new(|candidates: &[CandidateStrategy<NeuralStrategy>]| {
    if candidates.len() < 1 || rand::random::<f64>() < 0.4 {
      NeuralStrategy::new_random(&ghost_state, 16)
    } else {
      let mut improved = //candidates.choose (&mut thread_rng).clone();
      candidates.iter().enumerate().max_by_key(| (index, strategy) | {
        (strategy.playouts, -(*index as i32))
      }).unwrap().1.strategy.clone();

      for _ in 0..30 {
        improved.do_training_playout(&ghost_state);
      }
      improved
    }
  });*/

  //benchmark_step("Hexaghost (NeuralStrategy, training only)", & ghost_state, &mut neural_training_only);
  //benchmark_step("Hexaghost (NeuralStrategy, random/training)", & ghost_state, &mut neural_random_training);
}
