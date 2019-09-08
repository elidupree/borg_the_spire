use arrayvec::ArrayVec;
use ordered_float::OrderedFloat;
use rand::{seq::SliceRandom, Rng};
use std::collections::BTreeMap;

use crate::simulation::*;
use crate::simulation_state::*;

#[derive(Clone, Debug)]
pub struct Tree {
  initial_state: CombatState,
  root: ChoiceNode,
}

#[derive(Clone, Debug)]
pub struct ChoiceNode {
  total_score: f64,
  visits: usize,
  actions: Vec<(Action, ActionResults)>,
}

#[derive(Clone, Debug)]
pub struct ActionResults {
  total_score: f64,
  visits: usize,
  continuations: BTreeMap<Vec<i32>, ChoiceNode>,
}

#[derive(Clone, Debug)]
pub struct CombatResult {
  score: f64,
  hitpoints_left: i32,
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

impl ActionResults {
  fn new() -> ActionResults {
    ActionResults {
      visits: 0,
      total_score: 0.0,
      continuations: BTreeMap::new(),
    }
  }
  fn max_continuations(&self) -> usize {
    ((self.visits as f64).log2() + 1.5) as usize
  }
}

impl ChoiceNode {
  fn new() -> ChoiceNode {
    ChoiceNode {
      visits: 0,
      total_score: 0.0,
      actions: Vec::new(),
    }
  }
}

fn choose_action_naive(state: &CombatState) -> Action {
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

pub fn play_out<Strategy: Fn(&CombatState) -> Action>(
  state: &mut CombatState,
  runner: &mut impl Runner,
  strategy: Strategy,
) {
  while !state.combat_over() {
    let action = (strategy)(state);
    action.apply(state, runner);
  }
}

impl Tree {
  pub fn new(initial_state: CombatState) -> Tree {
    Tree {
      initial_state,
      root: ChoiceNode::new(),
    }
  }

  pub fn search_step(&mut self) -> f64 {
    let mut state = self.initial_state.clone();
    self.root.search_step(&mut state)
  }

  pub fn print_stuff(&self) {
    let mut actions: Vec<_> = self.root.actions.iter().collect();
    actions.sort_by_key(|(_action, results)| -(results.visits as i32));
    for (action, results) in actions {
      eprintln!(
        "{:?} {:.6} ({}) ",
        action,
        results.total_score / results.visits as f64,
        results.visits
      );
      if let Some((values, _)) = results.continuations.iter().next() {
        let mut runner = ReplayRunner::new(values);
        let mut state = self.initial_state.clone();
        action.apply(&mut state, &mut runner);

        eprintln!("arbitrary result of this action: {:#?}", state);
      }
    }

    self.root.print_stuff();
  }
}

impl ChoiceNode {
  fn print_stuff(&self) {
    eprintln!(
      "ChoiceNode {:.6} ({}), {} actions",
      self.total_score / self.visits as f64,
      self.visits,
      self.actions.len()
    );
    if let Some((action, results)) = self
      .actions
      .iter()
      .max_by_key(|(_action, results)| results.visits)
    {
      eprintln!(
        "Most tried: {:?} {:.6} ({}) ",
        action,
        results.total_score / results.visits as f64,
        results.visits
      );
      eprintln!(
        "  {}/{} outcomes",
        results.continuations.len(),
        results.max_continuations()
      );
      if let Some((values, node)) = results
        .continuations
        .iter()
        .max_by_key(|(_values, node)| node.visits)
      {
        eprintln!(" Most explored: {:?}", values);
        node.print_stuff();
      }
    }
  }

  fn search_step(&mut self, state: &mut CombatState) -> f64 {
    if state.combat_over() {
      return CombatResult::new(state).score;
    }

    if self.actions.is_empty() {
      for action in state.legal_actions() {
        self
          .actions
          .push ((action, ActionResults::new()));
      }

      play_out(state, &mut DefaultRunner::new(), choose_action_naive);
      return CombatResult::new(state).score;
    }

    self.visits += 1;

    let (candidate_action, results) = if let Some(unexplored) = self
      .actions
      .iter_mut()
      .find(|(_action, results)| results.visits == 0)
    {
      unexplored
    } else {
      // note: deviate from the usual MCTS formula by scaling the scores to the range [0,1],
      // so that it doesn't behave essentially randomly when there's only a small amount to gain
      // (e.g. a few hitpoints when you're guaranteed to win)
      let mut scores: ArrayVec<[_; 11]> = self
        .actions
        .iter()
        .map(| (_action, results) | OrderedFloat(results.total_score / results.visits as f64))
        .collect();
      let max_score = scores.iter().max().unwrap().0;
      let min_score = scores.iter().min().unwrap().0;
      if max_score > min_score {
        for score in scores.iter_mut() {
          *score = OrderedFloat((score.0 - min_score) / (max_score - min_score))
        }
      }
      let log_self_visits =(self.visits as f64).ln() ;
      self
        .actions
        .iter_mut()
        .zip(scores)
        .max_by_key(|((_action, results), score)| {
          OrderedFloat(score.0 + (2.0 *log_self_visits/ results.visits as f64).sqrt())
        })
        .unwrap()
        .0
    };

    results.visits += 1;

    let next_node;
    if results.continuations.len() < results.max_continuations() {
      let mut runner = DefaultRunner::new();
      candidate_action.apply(state, &mut runner);
      let generated_values = runner.into_generated_values();
      next_node = results
        .continuations
        .entry(generated_values)
        .or_insert_with(ChoiceNode::new);
    } else {
      let (values, node) = results
        .continuations
        .iter_mut()
        .min_by_key(|(_, node)| node.visits)
        .unwrap();

      let mut runner = ReplayRunner::new(values);
      candidate_action.apply(state, &mut runner);
      next_node = node;
    }
    let score = next_node.search_step(state);

    results.total_score += score;
    self.total_score += score;
    score
  }
}
