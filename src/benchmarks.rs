use std::time::{Instant, Duration};
use ordered_float::OrderedFloat;
use rand::seq::SliceRandom;

use crate::actions::*;
use crate::simulation::*;
use crate::simulation_state::*;
use crate::start_and_strategy_ai::{Strategy, FastStrategy, CombatResult, play_out};
use crate::neural_net_ai::NeuralStrategy;


pub trait StrategyOptimizer {
  type Strategy: Strategy;
  fn step (&mut self, state: & CombatState);
  fn report (&self)->& Self::Strategy;
}

struct CandidateStrategy <T> {
  strategy: T,
  playouts: usize,
  total_score: f64,
}

fn playout_result(state: & CombatState, strategy: & impl Strategy)->CombatResult {
  
      let mut state = state.clone();
      play_out (
        &mut Runner::new (&mut state, true, false),
        strategy,
      );
      CombatResult::new (& state)

}

pub struct ExplorationOptimizer <T, F> {
  candidate_strategies: Vec<CandidateStrategy <T>>,
  new_strategy: F,
  passes: usize,
  current_pass_index: usize,
}

impl <T, F> ExplorationOptimizer <T, F> {
  pub fn max_strategy_playouts(&self) -> usize {
    ((self.passes as f64).sqrt() + 2.0) as usize
  }
  
  pub fn new (new_strategy: F)->Self {
    ExplorationOptimizer {
      candidate_strategies: Vec::new(),
      new_strategy,
      passes: 0,
      current_pass_index: 0,
    }
  }
}


impl <T: Strategy, F: Fn (& [CandidateStrategy <T>])->T> StrategyOptimizer for ExplorationOptimizer <T, F> {
  type Strategy = T;
  fn step (&mut self, state: & CombatState) {
    loop {
      if self.current_pass_index >= self.candidate_strategies.len() {
        self.candidate_strategies.sort_by_key (| strategy | OrderedFloat (- strategy.total_score/strategy.playouts as f64));
        let mut index = 0;
        self.candidate_strategies.retain(| strategy | {
          index += 1;
          strategy.playouts >= index
        });
        
        self.passes += 1;
        self.candidate_strategies.push (CandidateStrategy {
          strategy: (self.new_strategy)(&self.candidate_strategies),
          playouts: 0,
          total_score: 0.0,
        });
        self.current_pass_index = 0;
      }
      
      let max_strategy_playouts = self.max_strategy_playouts();
      let strategy = &mut self.candidate_strategies [self.current_pass_index];
      self.current_pass_index += 1;
      
      if strategy.playouts < max_strategy_playouts {
        let result = playout_result(state, & strategy.strategy);
        strategy.total_score += result.score;
        strategy.playouts += 1;
        return
      }
    }
  }
  
  fn report (&self)->& Self::Strategy {
    let best = self.candidate_strategies.iter().find (| strategy | {
      // note that this function may be called in the middle of a pass, when the current best strategy has not yet been visited to increase its number of playouts to the new maximum, so allow a leeway of 1
      // since this function chooses the FIRST qualifying strategy, it's based on the most recent time the strategies were sorted, so this choice isn't biased by the change in score variance from some of them having one extra playout.
      strategy.playouts + 1 >= self.max_strategy_playouts()
    }).unwrap();
    
    println!( "ExplorationOptimizer reporting strategy with {} playouts, running average {}", best.playouts, (best.total_score/best.playouts as f64));
    
    & best.strategy
  }
}


impl StrategyOptimizer for NeuralStrategy {
  type Strategy = NeuralStrategy ;
  fn step (&mut self, state: & CombatState) {
    self.do_training_playout(state);
  }
  
  fn report (&self)->& Self::Strategy {
    self
  }
}

pub fn benchmark_step(name: & str, state: & CombatState, optimizer: &mut impl StrategyOptimizer) {
  println!( "Optimizing {}…", name);
  let start = Instant::now();
  let mut steps = 0;
  let elapsed = loop {
    optimizer.step(state);
    steps += 1;
    let elapsed = start.elapsed();
    if elapsed > Duration::from_millis(2000) {
      break elapsed;
    }
  };
  
  println!( "Optimized {} for {:.2?} ({} steps). Reporting…", name, elapsed, steps) ;
  let strategy = optimizer.report();
  
  let start = Instant::now();
  let mut steps = 0;
  let mut total_test_score = 0.0;
  let elapsed = loop {
    total_test_score += playout_result(state, strategy).score;
    steps += 1;
    
    let elapsed = start.elapsed();
    if elapsed > Duration::from_millis(500) {
      break elapsed;
    }
  };
  
  println!( "Evaluated {} for {:.2?} ({} playouts). Average score: {}", name, elapsed, steps, total_test_score / steps as f64) ;
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
}*/

pub fn run_benchmarks() {
  let optimization_playouts = 1000000;
  let test_playouts = 10000;
  let ghost_file = std::fs::File::open ("data/hexaghost.json").unwrap();
  let ghost_state: CombatState = serde_json::from_reader (std::io::BufReader::new (ghost_file)).unwrap();
  
  let mut fast_random: ExplorationOptimizer<FastStrategy, _> = ExplorationOptimizer::new (|_: &[CandidateStrategy <FastStrategy>] | FastStrategy::random());
  let mut fast_genetic: ExplorationOptimizer<FastStrategy, _> = ExplorationOptimizer::new (| candidates: & [CandidateStrategy <FastStrategy>] | {
    if candidates.len() < 2 {
      FastStrategy::random()
    }
    else {
      FastStrategy::offspring(& candidates.choose_multiple(&mut rand::thread_rng(), 2).map (| candidate | & candidate.strategy).collect::<Vec<_>>())
    }
  });
  
  let mut neural_random_only: ExplorationOptimizer<FastStrategy, _> = ExplorationOptimizer::new (|_: &[CandidateStrategy <FastStrategy>] | FastStrategy::random());
  let mut neural_training_only = NeuralStrategy::new_random(&ghost_state, 16);
  
  /*let mut neural_random_training: ExplorationOptimizer<NeuralStrategy, _> = ExplorationOptimizer::new (|_: &[CandidateStrategy <FastStrategy>] | {
    if candidates.len() < 1 || rand::random::>f64>() < 0.4 {
      NeuralStrategy::new_random(&ghost_state, 16)
    }
    else {
      let mut improved = 
    }
  });*/
  
  for _ in 0..20 {
    benchmark_step("Hexaghost (FastStrategy, random)", & ghost_state, &mut fast_random);
    benchmark_step("Hexaghost (FastStrategy, genetic)", & ghost_state, &mut fast_genetic);
    benchmark_step("Hexaghost (NeuralStrategy, random only)", & ghost_state, &mut neural_random_only);
    benchmark_step("Hexaghost (NeuralStrategy, training only)", & ghost_state, &mut neural_training_only);
  }
}

