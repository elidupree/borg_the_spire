//use arrayvec::ArrayVec;
use array_ext::Array;
use enum_map::EnumMap;
use ordered_float::OrderedFloat;
use rand::seq::SliceRandom;

use crate::actions::*;
use crate::ai_utils::{collect_starting_points, play_out, CombatResult, Strategy};
use crate::seed_system::TrivialSeed;
use crate::simulation::*;
use crate::simulation_state::*;
use rand::{Rng, SeedableRng};
use rand_pcg::Pcg64Mcg;

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
  pub fn new(parents: &[&'a T], rng: &mut impl Rng) -> OffspringBuilder<'a, T> {
    let mutation_rate: f64 = rng.gen::<f64>() * rng.gen::<f64>() * rng.gen::<f64>();
    let mut weighted_parents: Vec<_> = parents.iter().map(|parent| (*parent, rng.gen())).collect();
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

  pub fn combine_f64(&self, get: impl Fn(&T) -> f64, rng: &mut impl Rng) -> f64 {
    if rng.gen::<f64>() < self.mutation_rate {
      rng.gen()
    } else {
      get(
        &self
          .weighted_parents
          .choose_weighted(rng, |(_parent, weight)| *weight)
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

  pub fn random(rng: &mut impl Rng) -> FastStrategy {
    FastStrategy {
      card_priorities: EnumMap::from(|_| rng.gen()),
      monsters: Array::from_fn(|_| FastStrategyMonster {
        target_priority: rng.gen(),
      }),
      block_priority: rng.gen(),
    }
  }

  pub fn offspring(parents: &[&FastStrategy], rng: &mut impl Rng) -> FastStrategy {
    let builder = OffspringBuilder::new(parents, rng);

    FastStrategy {
      card_priorities: EnumMap::from(|card_id| {
        builder.combine_f64(|parent| parent.card_priorities[card_id], rng)
      }),
      monsters: Array::from_fn(|index| FastStrategyMonster {
        target_priority: builder.combine_f64(|parent| parent.monsters[index].target_priority, rng),
      }),
      block_priority: builder.combine_f64(|parent| parent.block_priority, rng),
    }
  }
}

#[derive(Clone, Debug)]
pub struct PurelyRandomStrategy;

impl Strategy for PurelyRandomStrategy {
  fn choose_choice(&self, state: &CombatState) -> Vec<Choice> {
    vec![state
      .legal_choices()
      .choose(&mut rand::thread_rng())
      .unwrap()
      .clone()]
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
      StandardRunner::new(&mut state, TrivialSeed::new(Pcg64Mcg::from_entropy()))
        .run_until_unable();
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
      strategy: FastStrategy::random(&mut rand::thread_rng()),
      visits: 0,
      total_score: 0.0,
    });

    for strategy in &mut self.candidate_strategies {
      if strategy.visits < max_strategy_visits {
        let mut state = self.state.clone();
        play_out(
          &mut StandardRunner::new(&mut state, TrivialSeed::new(Pcg64Mcg::from_entropy())),
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
