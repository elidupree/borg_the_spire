use crate::simulation::{run_until_unable, Choice, Runner};
use crate::simulation_state::CombatState;
use std::collections::{HashSet, VecDeque};

pub trait Strategy {
  fn choose_choice(&self, state: &CombatState) -> Vec<Choice>;
}

// This could use refinement on several issues – right now it incorrectly categorizes some deterministic choices as nondeterministic (e.g. drawing the one card left in your deck), and fails to deduplicate some identical sequences (e.g. strike-defend versus defend-strike when the second choice triggers something nondeterministic like unceasing top – choice.apply() skips right past the identical intermediate state)
pub fn collect_starting_points(
  state: CombatState,
  max_results: usize,
) -> Vec<(CombatState, Vec<Choice>)> {
  if state.combat_over() {
    return vec![(state.clone(), Vec::new())];
  }
  let mut frontier = VecDeque::new();
  let mut results = Vec::new();
  let mut discovered_midpoints = HashSet::new();
  frontier.push_back((state, Vec::new()));
  while let Some((state, history)) = frontier.pop_front() {
    if discovered_midpoints.insert(state.clone()) {
      let choices = state.legal_choices();
      for choice in choices {
        let mut new_state = state.clone();
        let mut runner = Runner::new(&mut new_state, false, false);
        runner.action_now(&choice);
        run_until_unable(&mut runner);
        let mut new_history = history.clone();
        new_history.push(choice.clone());
        assert!(new_state.fresh_subaction_queue.is_empty());
        if (results.len() + frontier.len()) < max_results
          && !new_state.combat_over()
          && new_state.stale_subaction_stack.is_empty()
        {
          assert!(new_state.actions.is_empty());
          frontier.push_back((new_state, new_history));
        } else {
          results.push((new_state, new_history));
        }
      }
    }
  }
  results
}

pub fn play_out<S: Strategy>(runner: &mut Runner, strategy: &S) {
  run_until_unable(runner);
  while !runner.state().combat_over() {
    let choices = strategy.choose_choice(runner.state());
    for choice in choices {
      assert!(runner.state().fresh_subaction_queue.is_empty());
      assert!(runner.state().stale_subaction_stack.is_empty());
      assert!(runner.state().actions.is_empty());
      runner.action_now(&choice);
      run_until_unable(runner);
    }
  }
}

#[derive(Clone, Debug)]
pub struct CombatResult {
  pub score: f64,
  pub hitpoints_left: i32,
}

impl CombatResult {
  pub fn new(state: &CombatState) -> CombatResult {
    if state.player.creature.hitpoints > 0 {
      // TODO punish for stolen gold
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