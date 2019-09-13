//use arrayvec::ArrayVec;
use ordered_float::OrderedFloat;
//use rand::{seq::SliceRandom, Rng};
use std::collections::{HashSet, VecDeque};

use crate::simulation::*;
use crate::simulation_state::*;

pub trait Strategy {
  fn choose_choice(&self, state: &CombatState) -> Vec<Choice>;
}

#[derive(Clone, Debug)]
pub struct SearchState {
  pub initial_state: CombatState,
  pub visits: usize,
  pub starting_points: Vec<StartingPoint>,
}

#[derive(Clone, Debug)]
pub struct StartingPoint {
  pub state: CombatState,
  pub choices: Vec<Choice>,
  pub candidate_strategies: Vec<CandidateStrategy>,
  pub visits: usize,
}

#[derive(Clone, Debug)]
pub struct CandidateStrategy {
  pub strategy: SomethingStrategy,
  pub visits: usize,
  pub total_score: f64,
}

#[derive(Clone, Debug)]
pub struct SomethingStrategy {}

impl Strategy for SomethingStrategy {
  fn choose_choice(&self, state: &CombatState) -> Vec<Choice> {
    /*let legal_choices = state.legal_choices();

    if legal_choices.len() == 1 || rand::thread_rng().gen_bool(0.00001) {
      Choice::EndTurn
    } else {
      legal_choices[1..]
        .choose(&mut rand::thread_rng())
        .unwrap()
        .clone()
    }*/

    let combos = collect_starting_points(state.clone(), 200);
    let choices = combos.into_iter().map(|(mut state, choices)| {
      run_until_unable(&mut DefaultRunner::new(&mut state));
      let score = self.evaluate(&state);
      (choices, score)
    });
    choices
      .max_by_key(|(_, score)| OrderedFloat(*score))
      .unwrap()
      .0
  }
}

impl SomethingStrategy {
  pub fn evaluate(&self, state: &CombatState) -> f64 {
    let mut result = 0.0;
    result += state.player.creature.hitpoints as f64;
    for monster in &state.monsters {
      if !monster.gone {
        result -= 3.0;
        result -= monster.creature.hitpoints as f64 * 0.1;
      }
    }
    result
  }
}

pub fn new_random_strategy() -> SomethingStrategy {
  SomethingStrategy {}
}

// This could use refinement on several issues – right now it incorrectly categorizes some deterministic choices as nondeterministic (e.g. drawing the one card left in your deck), and fails to deduplicate some identical sequences (e.g. strike-defend versus defend-strike when the second choice triggers something nondeterministic like unceasing top – choice.apply() skips right past the identical intermediate state)
pub fn collect_starting_points(
  state: CombatState,
  max_results: usize,
) -> Vec<(CombatState, Vec<Choice>)> {
  let mut frontier = VecDeque::new();
  let mut results = Vec::new();
  let mut discovered_midpoints = HashSet::new();
  frontier.push_back((state, Vec::new()));
  while let Some((state, history)) = frontier.pop_front() {
    if discovered_midpoints.insert(state.clone()) {
      let choices = state.legal_choices();
      for choice in choices {
        let mut new_state = state.clone();
        let mut runner = DeterministicRunner::new(&mut new_state);
        runner.apply(&choice);
        let mut new_history = history.clone();
        new_history.push(choice.clone());
        if (results.len() + frontier.len()) < max_results
          && !new_state.combat_over()
          && new_state.fresh_action_queue.is_empty()
        {
          frontier.push_back((new_state, new_history));
        } else {
          results.push((new_state, new_history));
        }
      }
    }
  }
  results
}

impl SearchState {
  pub fn new(initial_state: CombatState) -> SearchState {
    let starts = collect_starting_points(initial_state.clone(), 1000);

    SearchState {
      initial_state,
      visits: 0,
      starting_points: starts
        .into_iter()
        .map(|(state, choices)| StartingPoint {
          state,
          choices,
          candidate_strategies: Vec::new(),
          visits: 0,
        })
        .collect(),
    }
  }

  pub fn search_step(&mut self) {
    self.visits += 1;
    for starting_point in &mut self.starting_points {
      starting_point.search_step();
    }
    self
      .starting_points
      .sort_by_key(|start| OrderedFloat(-start.score()));
  }
}

impl StartingPoint {
  pub fn max_strategy_visits(&self) -> usize {
    ((self.visits as f64).sqrt() + 2.0) as usize
  }

  pub fn search_step(&mut self) {
    self.visits += 1;
    let max_strategy_visits = self.max_strategy_visits();
    self.candidate_strategies.push(CandidateStrategy {
      strategy: new_random_strategy(),
      visits: 0,
      total_score: 0.0,
    });

    for strategy in &mut self.candidate_strategies {
      if strategy.visits < max_strategy_visits {
        let mut state = self.state.clone();
        play_out(&mut DefaultRunner::new(&mut state), &strategy.strategy);
        let result = CombatResult::new(&state);
        strategy.total_score += result.score;
        strategy.visits += 1;
      }
    }

    self
      .candidate_strategies
      .sort_by_key(|strategy| OrderedFloat(-strategy.total_score / strategy.visits as f64));
    for (index, strategy) in self.candidate_strategies.iter_mut().enumerate() {
      if strategy.visits <= index {
        strategy.visits = usize::max_value();
      }
    }
    self
      .candidate_strategies
      .retain(|strategy| strategy.visits != usize::max_value());
  }

  pub fn score(&self) -> f64 {
    self
      .candidate_strategies
      .iter()
      .find(|strategy| strategy.visits == self.max_strategy_visits())
      .map(|strategy| strategy.total_score / strategy.visits as f64)
      .unwrap_or(0.0)
  }
}

pub fn play_out<S: Strategy>(runner: &mut impl Runner, strategy: &S) {
  run_until_unable(runner);
  while !runner.state().combat_over() {
    let choices = strategy.choose_choice(runner.state());
    for choice in choices {
      assert!(runner.state().fresh_action_queue.is_empty());
      assert!(runner.state().stale_action_stack.is_empty());
      runner.apply(&choice);
    }
  }
}

#[derive(Clone, Debug)]
pub struct CombatResult {
  pub score: f64,
  pub hitpoints_left: i32,
}

impl CombatResult {
  fn new(state: &CombatState) -> CombatResult {
    if state.player.creature.hitpoints > 0 {
      CombatResult {
        score: 1.0 + state.player.creature.hitpoints as f64 * 0.0001,
        hitpoints_left: state.player.creature.hitpoints,
      }
    } else {
      CombatResult {
        score: 0.0
          - state
            .monsters
            .iter()
            .map(|monster| monster.creature.hitpoints)
            .sum::<i32>() as f64
            * 0.000001,
        hitpoints_left: 0,
      }
    }
  }
}
