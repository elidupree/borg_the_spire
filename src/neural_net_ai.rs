use enum_map::{Enum, EnumMap};
use ordered_float::OrderedFloat;
use rand::seq::SliceRandom;
use rand::{random, Rng, SeedableRng};
use rand_pcg::Pcg64Mcg;
use std::cmp::min;
use std::iter;

use crate::actions::*;
use crate::ai_utils::{CombatResult, Strategy};
use crate::seed_system::TrivialSeed;
use crate::simulation::*;
use crate::simulation_state::*;

/*

A neural-network-based AI.

The network gets a large number of inputs corresponding to the game state,
and one output layer that has one node per possible move (i.e. each exact card you could possibly play and End Turn and choices from True Grit and such...)

Having a full matrix of weights between these 2 layers would be too large, so we also have one internal layer with only a small number of nodes.

The output numbers are intended to be: *given making this move in this game state, what is the average score at the end of the encounter?*

Thus, the way to backpropagate is: when you complete an encounter, for each move you chose in that encounter, tweak all of the weights in order to adjust the output for that choice *towards* the resulting score, proportional to the difference between the current output and the resulting score. In theory, if the tweak amounts are small enough, this would cause the output to converge towards the average resulting score.

"But wait," you say – "Suppose that, when you initially generate the random network, it is biased towards giving an overly low score for most possible moves. In this case, all your playouts will score "better than expected", meaning that the network will reinforce the idea that these random moves are the best ones."

So here's what we do: for each choice, we introduce a small chance of playing *any* of the possible moves – a high chance for the one the network thinks is best, and a low chance for the others. Moreover, when we tweak the network, we make the tweak size proportional to the reciprocal of the probability of choosing the move we chose. That way, on *average* over many playouts, we do the same total amount of tweaking for each possible move, even though some are played much more often than others.

*/

#[derive(Clone, Debug)]
pub struct NeuralStrategy {
  hidden_layer_size: usize,
  input_weights: Vec<Vec<f64>>,
  choice_weights: ChoiceWeights,
}

#[derive(Clone, Debug)]
struct ChoiceWeights {
  play_card_weights: EnumMap<CardId, [Vec<f64>; 2]>,
  end_turn_weights: Vec<f64>,
}

#[derive(Clone, Debug)]
struct ChoiceAnalysis {
  choice: Choice,
  score: f64,
  selection_probability: f64,
}

#[derive(Clone, Debug)]
struct CombatStateAnalysis {
  inputs: Vec<f64>,
  hidden_inputs: Vec<f64>,
  hidden_outputs: Vec<f64>,
  choices: Vec<ChoiceAnalysis>,
}

impl Strategy for NeuralStrategy {
  fn choose_choice(&self, state: &CombatState) -> Vec<Choice> {
    let analysis = self.analyze(state);

    let best_choice = analysis
      .choices
      .iter()
      // this impl just plays and doesn't analyze it, so no need to use the selection probabilities (which allow a chance of making a mistake)
      .max_by_key(|choice| OrderedFloat(choice.score))
      .unwrap();

    vec![best_choice.choice.clone()]
  }
}

const MIN_SELECTION_PROBABILITY: f64 = 0.001;

fn logistic(x: f64) -> f64 {
  1.0 / (1.0 + (-x).exp())
}

impl ChoiceWeights {
  fn get(&self, choice: &Choice) -> &[f64] {
    match choice {
      Choice::PlayCard (PlayCard {card, target: _}) => {
        &self.play_card_weights [card.card_info.id] [min (card.upgrades as usize, 1)]
      }
      Choice::EndTurn (_) => {
        &self.end_turn_weights
      }
      _=> panic!("NeuralStrategy doesn't yet know how to handle choices other than playing cards and ending turn"),
    }
  }
  fn get_mut(&mut self, choice: &Choice) -> &mut [f64] {
    match choice {
      Choice::PlayCard (PlayCard {card, target: _}) => {
        &mut self.play_card_weights [card.card_info.id] [min (card.upgrades as usize, 1)]
      }
      Choice::EndTurn (_) => {
        &mut self.end_turn_weights
      }
      _=> panic!("NeuralStrategy doesn't yet know how to handle choices other than playing cards and ending turn"),
    }
  }
}

fn push_creature_inputs(result: &mut Vec<f64>, creature: &Creature) {
  result.push(creature.hitpoints as f64 / creature.max_hitpoints as f64);
  result.push(creature.block as f64 / creature.max_hitpoints as f64);
  let powers_start = result.len();
  result.extend_from_slice(&PowerId::from_function(|_| 0.0));

  for power in &creature.powers {
    result[powers_start + Enum::<f64>::to_usize(power.power_id)] += logistic(power.amount as f64);
  }
}

fn inputs(state: &CombatState) -> Vec<f64> {
  let mut result = Vec::with_capacity(100);

  result.push(logistic(state.turn_number as f64));

  result.push(state.player.energy as f64);
  push_creature_inputs(&mut result, &state.player.creature);

  for monster_index in 0..MAX_MONSTERS {
    let monster_start = result.len();
    // the number of monsters may change, so we always write a monster, but zero it out if it's not real
    let monster = state
      .monsters
      .get(monster_index)
      .unwrap_or_else(|| &state.monsters[0]);

    result.push(1.0); // "alive" flag
    result.push(monster.innate_damage_amount.unwrap_or(0) as f64);
    result.push(logistic(
      monster.move_history.get(0).copied().unwrap_or(0) as f64 / 10.0,
    ));
    result.push(logistic(
      monster.move_history.get(1).copied().unwrap_or(0) as f64 / 10.0,
    ));
    push_creature_inputs(&mut result, &monster.creature);

    // retroactively zeroing instead of writing zeros separately, to guarantee that I don't accidentally put the wrong number of values
    if monster.gone || monster_index >= state.monsters.len() {
      for input in &mut result[monster_start..] {
        *input = 0.0;
      }
    }
  }

  for cards in &[
    state.hand.as_slice(),
    state.draw_pile.as_slice(),
    state.discard_pile.as_slice(),
    state.exhaust_pile.as_slice(),
  ] {
    let normal_start = result.len();
    result.extend_from_slice(&CardId::from_function(|_| 0.0));
    let upgraded_start = result.len();
    result.extend_from_slice(&CardId::from_function(|_| 0.0));
    for card in *cards {
      let start = if card.upgrades > 0 {
        upgraded_start
      } else {
        normal_start
      };
      result[start + Enum::<f64>::to_usize(card.card_info.id)] += 0.1;
    }
  }

  result
}

fn random_weight() -> f64 {
  rand::thread_rng().gen_range(-1.0..1.0)
}

fn random_weights(hidden_layer_size: usize) -> Vec<f64> {
  (0..hidden_layer_size).map(|_| random_weight()).collect()
}

impl NeuralStrategy {
  pub fn new_random(hidden_layer_size: usize) -> Self {
    let inputs = inputs(&CombatState::default());
    let input_weights = inputs
      .iter()
      .map(|_| random_weights(hidden_layer_size))
      .collect();
    let play_card_weights = EnumMap::from(|_| {
      [
        random_weights(hidden_layer_size),
        random_weights(hidden_layer_size),
      ]
    });
    let end_turn_weights = random_weights(hidden_layer_size);

    NeuralStrategy {
      hidden_layer_size,
      input_weights,
      choice_weights: ChoiceWeights {
        play_card_weights,
        end_turn_weights,
      },
    }
  }

  pub fn mutated(&self) -> Self {
    let mut result = self.clone();

    let mutation_rate = random::<f64>() * random::<f64>() * random::<f64>();

    for weight in result
      .input_weights
      .iter_mut()
      .flatten()
      .chain(result.choice_weights.end_turn_weights.iter_mut())
      .chain(
        result
          .choice_weights
          .play_card_weights
          .iter_mut()
          .flat_map(|(_id, weights)| weights.iter_mut().flatten()),
      )
    {
      if random::<f64>() < mutation_rate {
        *weight = random_weight();
      }
    }

    result
  }

  fn analyze(&self, state: &CombatState) -> CombatStateAnalysis {
    let inputs = inputs(state);

    let mut hidden_inputs: Vec<f64> = iter::repeat(0.0).take(self.hidden_layer_size).collect();

    for (input_index, input) in inputs.iter().enumerate() {
      if *input == 0.0 {
        continue;
      }
      for (hidden_index, weight) in self.input_weights[input_index].iter().enumerate() {
        hidden_inputs[hidden_index] += input * weight;
      }
    }

    let hidden_outputs: Vec<f64> = hidden_inputs.iter().copied().map(logistic).collect();

    let legal_choices = state.legal_choices();
    let mut choices: Vec<_> = legal_choices
      .into_iter()
      .map(|choice| {
        let score = self
          .choice_weights
          .get(&choice)
          .iter()
          .enumerate()
          .map(|(hidden_index, weight)| hidden_outputs[hidden_index] * weight)
          .sum::<f64>();

        ChoiceAnalysis {
          choice,
          score,
          selection_probability: MIN_SELECTION_PROBABILITY,
        }
      })
      .collect();

    choices
      .iter_mut()
      .max_by_key(|choice| OrderedFloat(choice.score))
      .unwrap()
      .selection_probability = 1.0 - ((choices.len() as f64 - 1.0) * MIN_SELECTION_PROBABILITY);

    CombatStateAnalysis {
      inputs,
      hidden_inputs,
      hidden_outputs,
      choices,
    }
  }

  fn backpropagate(
    &self,
    analysis: &CombatStateAnalysis,
    new_version: &mut Self,
    choice: &ChoiceAnalysis,
    observed_score: f64,
  ) {
    let learning_rate = MIN_SELECTION_PROBABILITY / choice.selection_probability;
    // desired_change_size = observed_score - choice.score;
    // error = desired_change_size^2
    // = choice.score^2 - 2*observed_score*choice.score + observed_score^2
    // derror/dscore = 2*choice.score - 2*observed_score
    // and the 2 is just a constant factor that we can adjust (since error is in arbitrary units) to apply the learning rate
    let derror_dscore = (choice.score - observed_score) * learning_rate;

    let old_weights = self.choice_weights.get(&choice.choice);
    let new_weights = new_version.choice_weights.get_mut(&choice.choice);

    for (hidden_index, hidden_output) in analysis.hidden_outputs.iter().enumerate() {
      let dscore_dweight = hidden_output;
      let derror_dweight = derror_dscore * dscore_dweight;
      new_weights[hidden_index] -= derror_dweight;

      let dscore_dhiddenoutput = old_weights[hidden_index];
      let derror_dhiddenoutput = derror_dscore * dscore_dhiddenoutput;

      // derivative assuming it was computed by the logistic function:
      let dhiddenoutput_dhiddeninput = hidden_output * (1.0 - hidden_output);
      let derror_dhiddeninput = derror_dhiddenoutput * dhiddenoutput_dhiddeninput;
      for (input_index, input) in analysis.inputs.iter().enumerate() {
        let dscore_dweight = input;
        let derror_dweight = derror_dhiddeninput * dscore_dweight;
        new_version.input_weights[input_index][hidden_index] -= derror_dweight;
      }
    }
  }

  pub fn do_training_playout(&mut self, state: &CombatState) {
    let mut playout_state = state.clone();
    let mut runner = StandardRunner::new(
      &mut playout_state,
      TrivialSeed::new(Pcg64Mcg::from_entropy()),
      false,
    );
    let mut analyses: Vec<(CombatStateAnalysis, ChoiceAnalysis)> = Vec::new();

    run_until_unable(&mut runner);
    while !runner.state().combat_over() {
      let analysis = self.analyze(runner.state());

      let best_choice = analysis
        .choices
        .choose_weighted(&mut rand::thread_rng(), |choice| {
          choice.selection_probability
        })
        .unwrap()
        .clone();

      runner.apply_choice(&best_choice.choice);

      analyses.push((analysis, best_choice));
    }

    let result = CombatResult::new(&playout_state);

    let mut new_version = self.clone();

    for (analysis, choice_made) in analyses {
      self.backpropagate(&analysis, &mut new_version, &choice_made, result.score);
    }

    *self = new_version;
  }
}
