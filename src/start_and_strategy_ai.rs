//use arrayvec::ArrayVec;
use array_ext::Array;
use enum_map::EnumMap;
use ordered_float::OrderedFloat;
use rand::seq::SliceRandom;
use std::collections::{HashSet, VecDeque};

use crate::actions::*;
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
  pub strategy: FastStrategy,
  pub visits: usize,
  pub total_score: f64,
}

#[derive(Clone, Debug)]
pub struct FastStrategy {
  card_priorities: EnumMap<CardId, f64>,
  monsters: [FastStrategyMonster; MAX_MONSTERS],
  block_priority: f64,
}
#[derive(Clone, Debug)]
pub struct FastStrategyMonster {
  target_priority: f64,
}

impl Strategy for FastStrategy {
  fn choose_choice(&self, state: &CombatState) -> Vec<Choice> {
    let legal_choices = state.legal_choices();

    let incoming_damage = state
      .monsters
      .iter()
      .enumerate()
      .map(|(index, monster)| {
        if monster.gone {
          0
        } else {
          crate::simulation_state::monsters::intent_actions(state, index)
            .into_iter()
            .map(|action| {
              if let DynAction::DamageAction(action) = action {
                action.info.output
              } else {
                0
              }
            })
            .sum::<i32>()
        }
      })
      .sum::<i32>()
      - state.player.creature.block;

    vec![legal_choices
      .into_iter()
      .max_by_key(|choice| OrderedFloat(self.evaluate(state, choice, incoming_damage)))
      .unwrap()]
  }
}

pub struct OffspringBuilder<'a, T> {
  weighted_parents: Vec<(&'a T, f64)>,
  mutation_rate: f64,
}

impl<'a, T> OffspringBuilder<'a, T> {
  pub fn new(parents: &[&'a T]) -> OffspringBuilder<'a, T> {
    let mutation_rate: f64 = rand::random::<f64>() * rand::random::<f64>() * rand::random::<f64>();
    let mut weighted_parents: Vec<_> = parents
      .iter()
      .map(|parent| (*parent, rand::random()))
      .collect();
    let total_weight: f64 = weighted_parents
      .iter()
      .map(|(_parent, weight)| weight)
      .sum();
    for (_parent, weight) in &mut weighted_parents {
      *weight /= total_weight;
    }

    OffspringBuilder {
      weighted_parents,
      mutation_rate,
    }
  }

  pub fn combine_f64(&self, get: impl Fn(&T) -> f64) -> f64 {
    if rand::random::<f64>() < self.mutation_rate {
      rand::random()
    } else {
      get(
        &self
          .weighted_parents
          .choose_weighted(&mut rand::thread_rng(), |(_parent, weight)| *weight)
          .unwrap()
          .0,
      )
    }
  }
}

impl FastStrategy {
  pub fn evaluate(&self, state: &CombatState, choice: &Choice, incoming_damage: i32) -> f64 {
    match choice {
      Choice::EndTurn(_) => 0.0,
      Choice::PlayCard(PlayCard { card, target }) => {
        let mut result = self.card_priorities[card.card_info.id];
        for action in crate::simulation_state::cards::card_actions(state, card.clone(), *target) {
          match action {
            DynAction::DamageAction(_action) => {}
            DynAction::GainBlockAction(action) => {
              result +=
                std::cmp::min(action.amount, incoming_damage) as f64 * self.block_priority * 0.1;
            }
            _ => {}
          }
        }
        if card.card_info.has_target {
          result += self.monsters[*target].target_priority * 0.000001;
        }
        result
      }
      _ => 0.0,
    }
  }

  pub fn random() -> FastStrategy {
    FastStrategy {
      card_priorities: EnumMap::from(|_| rand::random()),
      monsters: Array::from_fn(|_| FastStrategyMonster {
        target_priority: rand::random(),
      }),
      block_priority: rand::random(),
    }
  }

  pub fn offspring(parents: &[&FastStrategy]) -> FastStrategy {
    let builder = OffspringBuilder::new(parents);

    FastStrategy {
      card_priorities: EnumMap::from(|card_id| {
        builder.combine_f64(|parent| parent.card_priorities[card_id])
      }),
      monsters: Array::from_fn(|index| FastStrategyMonster {
        target_priority: builder.combine_f64(|parent| parent.monsters[index].target_priority),
      }),
      block_priority: builder.combine_f64(|parent| parent.block_priority),
    }
  }
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
      run_until_unable(&mut Runner::new(&mut state, true, false));
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
      strategy: FastStrategy::random(),
      visits: 0,
      total_score: 0.0,
    });

    for strategy in &mut self.candidate_strategies {
      if strategy.visits < max_strategy_visits {
        let mut state = self.state.clone();
        play_out(
          &mut Runner::new(&mut state, true, false),
          &strategy.strategy,
        );
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
