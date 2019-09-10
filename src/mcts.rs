use arrayvec::ArrayVec;
use ordered_float::OrderedFloat;
use rand::{seq::SliceRandom, Rng};
use std::collections::BTreeMap;

use crate::simulation::*;
use crate::simulation_state::*;

#[derive(Clone, Debug)]
pub struct SearchTree {
  pub initial_state: CombatState,
  pub root: ChoiceNode,
}

#[derive(Clone, Debug)]
pub struct ChoiceNode {
  pub total_score: f64,
  pub visits: usize,
  pub choices: Vec<(Choice, ChoiceResults)>,
}

#[derive(Clone, Debug)]
pub struct ChoiceResults {
  pub total_score: f64,
  pub visits: usize,
  pub continuations: BTreeMap<Replay, ChoiceNode>,
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

impl ChoiceResults {
  pub fn new() -> ChoiceResults {
    ChoiceResults {
      visits: 0,
      total_score: 0.0,
      continuations: BTreeMap::new(),
    }
  }
  pub fn max_continuations(&self) -> usize {
    ((self.visits as f64).log2() + 1.5) as usize
  }
}

impl ChoiceNode {
  pub fn new() -> ChoiceNode {
    ChoiceNode {
      visits: 0,
      total_score: 0.0,
      choices: Vec::new(),
    }
  }
}

fn choose_choice_naive(state: &CombatState) -> Choice {
  let legal_choices = state.legal_choices();

  if legal_choices.len() == 1 || rand::thread_rng().gen_bool(0.00001) {
    Choice::EndTurn
  } else {
    legal_choices[1..]
      .choose(&mut rand::thread_rng())
      .unwrap()
      .clone()
  }
}

pub fn play_out<Strategy: Fn(&CombatState) -> Choice>(
  state: &mut CombatState,
  runner: &mut impl Runner,
  strategy: Strategy,
) {
  while !state.combat_over() {
    let choice = (strategy)(state);
    choice.apply(state, runner);
  }
}

impl SearchTree {
  pub fn new(initial_state: CombatState) -> SearchTree {
    SearchTree {
      initial_state,
      root: ChoiceNode::new(),
    }
  }

  pub fn search_step(&mut self) -> f64 {
    let mut state = self.initial_state.clone();
    self.root.search_step(&mut state)
  }

  pub fn print_stuff(&self) {
    let mut choices: Vec<_> = self.root.choices.iter().collect();
    choices.sort_by_key(|(_choice, results)| -(results.visits as i32));
    for (choice, results) in choices {
      eprintln!(
        "{:?} {:.6} ({}) ",
        choice,
        results.total_score / results.visits as f64,
        results.visits
      );
      if let Some((replay, _)) = results.continuations.iter().next() {
        let mut state = self.initial_state.clone();
        replay_choice (&mut state, choice, replay);

        eprintln!("arbitrary result of this choice: {:#?}", state);
      }
    }

    self.root.print_stuff();
  }
}

impl ChoiceNode {
  pub fn print_stuff(&self) {
    eprintln!(
      "ChoiceNode {:.6} ({}), {} choices",
      self.total_score / self.visits as f64,
      self.visits,
      self.choices.len()
    );
    if let Some((choice, results)) = self
      .choices
      .iter()
      .max_by_key(|(_choice, results)| results.visits)
    {
      eprintln!(
        "Most tried: {:?} {:.6} ({}) ",
        choice,
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

  pub fn search_step(&mut self, state: &mut CombatState) -> f64 {
    if state.combat_over() {
      return CombatResult::new(state).score;
    }

    if self.choices.is_empty() {
      for choice in state.legal_choices() {
        self
          .choices
          .push ((choice, ChoiceResults::new()));
      }

      play_out(state, &mut DefaultRunner::new(), choose_choice_naive);
      return CombatResult::new(state).score;
    }

    self.visits += 1;

    let (candidate_choice, results) = if let Some(unexplored) = self
      .choices
      .iter_mut()
      .find(|(_choice, results)| results.visits == 0)
    {
      unexplored
    } else {
      // note: deviate from the usual MCTS formula by scaling the scores to the range [0,1],
      // so that it doesn't behave essentially randomly when there's only a small amount to gain
      // (e.g. a few hitpoints when you're guaranteed to win)
      let mut scores: ArrayVec<[_; 11]> = self
        .choices
        .iter()
        .map(| (_choice, results) | OrderedFloat(results.total_score / results.visits as f64))
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
        .choices
        .iter_mut()
        .zip(scores)
        .max_by_key(|((_choice, results), score)| {
          OrderedFloat(score.0 + (2.0 *log_self_visits/ results.visits as f64).sqrt())
        })
        .unwrap()
        .0
    };

    results.visits += 1;

    let next_node;
    if results.continuations.len() < results.max_continuations() {
      let mut runner = DefaultRunner::new();
      candidate_choice.apply(state, &mut runner);
      let generated_values = runner.into_replay();
      next_node = results
        .continuations
        .entry(generated_values)
        .or_insert_with(ChoiceNode::new);
    } else {
      let (replay, node) = results
        .continuations
        .iter_mut()
        .min_by_key(|(_, node)| node.visits)
        .unwrap();

      replay_choice (state, candidate_choice, replay);
      next_node = node;
    }
    let score = next_node.search_step(state);

    results.total_score += score;
    self.total_score += score;
    score
  }
}
