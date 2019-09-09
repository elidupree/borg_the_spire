use arrayvec::ArrayVec;
use ordered_float::OrderedFloat;
use rand::{seq::SliceRandom, Rng};
use std::collections::{HashSet, VecDeque};

use crate::simulation::*;
use crate::simulation_state::*;

pub trait Strategy {
  fn choose_action (&self, state: & CombatState)->Action;
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
  pub actions: Vec<Action>,
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
pub struct SomethingStrategy {

}

impl Strategy for SomethingStrategy {
  fn choose_action (&self, state: & CombatState)->Action {
    let legal_actions = state.legal_actions();

  if legal_actions.len() == 1 || rand::thread_rng().gen_bool(0.00001) {
    Action::EndTurn
  } else {
    legal_actions[1..]
      .choose(&mut rand::thread_rng())
      .unwrap()
      .clone()
  }
  }
}

pub fn new_random_strategy()->SomethingStrategy {
  SomethingStrategy {}
}

// This could use refinement on several issues – right now it incorrectly categorizes some deterministic actions as nondeterministic (e.g. drawing the one card left in your deck), and fails to deduplicate some identical sequences (e.g. strike-defend versus defend-strike when the second action triggers something nondeterministic like unceasing top – action.apply() skips right past the identical intermediate state)
pub fn collect_starting_points (state: CombatState, max_results: usize)->Vec <(CombatState, Vec<Action>)> {

  let mut frontier = VecDeque::new();
  let mut results = Vec::new();
  let mut discovered_midpoints = HashSet::new();
  frontier.push_back ((state, Vec::new()));
  while let Some ((state, history)) = frontier.pop_front() {
  
  if discovered_midpoints.insert (state.clone()) {
      let actions = state.legal_actions();
  for action in actions {
    let mut new_state = state.clone() ;
    let mut runner = DefaultRunner::new();
    action.apply (&mut new_state, &mut runner);
    let mut new_history = history.clone() ;
    new_history.push (action.clone()) ;
    if runner.into_replay().generated_values.is_empty() &&!new_state.combat_over() && (results.len() + frontier.len()) < max_results {
      frontier.push_back ((new_state, new_history)) ;
    }
    else {
      results.push ((state.clone(), new_history));
    }
  }

    }

  
  }
  results
}

impl SearchState {
  pub fn new(initial_state: CombatState) -> SearchState {
    let mut starts = collect_starting_points (initial_state.clone(), 1000);
    
    SearchState {
      initial_state,
      visits: 0,
      starting_points: starts.into_iter().map (| (state, actions) | StartingPoint {
        state, actions,
        candidate_strategies: Vec::new(),
        visits: 0,
      }).collect(),
    }
  }

  pub fn search_step (&mut self) {
    self.visits += 1;
    for starting_point in &mut self.starting_points {
      starting_point.search_step();
    }
    self.starting_points.sort_by_key (| start | OrderedFloat (- start.score()));
  }
}

impl StartingPoint {
  pub fn max_strategy_visits (&self)->usize {
    ((self.visits as f64).sqrt() + 2.0) as usize
  }
  
  pub fn search_step (&mut self) {
    self.visits += 1;
    let max_strategy_visits = self.max_strategy_visits();
    self.candidate_strategies.push (CandidateStrategy {
strategy: new_random_strategy(), visits: 0, total_score: 0.0,
});

    for strategy in &mut self.candidate_strategies {
      if strategy.visits <max_strategy_visits {
      let mut state = self.state.clone();
      play_out (&mut state, &mut DefaultRunner::new(), & strategy.strategy) ;
      let result = CombatResult::new (& state) ;
      strategy.total_score += result.score;
      strategy.visits += 1;
      }
    }
    
    self.candidate_strategies.sort_by_key (| strategy | OrderedFloat (- strategy.total_score/strategy.visits as f64));
    for (index, strategy) in self.candidate_strategies.iter_mut().enumerate() {
      if strategy.visits <= index {
        strategy.visits = usize::max_value();
      }
    }
    self.candidate_strategies.retain (| strategy | strategy.visits != usize::max_value());
  }

pub fn score (&self)->f64 {
  self.candidate_strategies.iter().find (| strategy | strategy.visits == self.max_strategy_visits()).map (| strategy | {
    strategy.total_score/strategy.visits as f64
  }).unwrap_or (0.0)
}
}


pub fn play_out<S: Strategy>(
  state: &mut CombatState,
  runner: &mut impl Runner,
  strategy: & S,
) {
  while !state.combat_over() {
    let action = strategy.choose_action (state);
    action.apply(state, runner);
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

