use std::cmp::min;
use std::iter;
use enum_map::EnumMap;
use ordered_float::OrderedFloat;

use crate::actions::*;
use crate::simulation::*;
use crate::simulation_state::*;
use crate::start_and_strategy_ai::{Strategy, CombatResult};


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


#[derive (Clone, Debug)]
pub struct NeuralStrategy {
  hidden_layer_size: usize,
  input_weights: Vec<Vec<f64>>,
  choice_weights: ChoiceWeights,
}

#[derive (Clone, Debug)]
struct ChoiceWeights {
  play_card_weights: EnumMap <CardId, [Vec<f64>; 2]>,
  end_turn_weights: Vec<f64>,
}

struct ChoiceAnalysis {
  choice: Choice,
  score: f64,
  selection_probability: f64,
}

struct CombatStateAnalysis {
  inputs: Vec<f64>,
  hidden_inputs: Vec<f64>,
  hidden_outputs: Vec<f64>,
  choices: Vec<ChoiceAnalysis>,
}


impl Strategy for NeuralStrategy {
  fn choose_choice (&self, state: & CombatState)->Vec<Choice> {
    let analysis = self.analyze (state);
    
    let best_choice = analysis.choices
      .iter()
      // this impl just plays and doesn't analyze it, so no need to use the selection probabilities (which allow a chance of making a mistake)
      .max_by_key (| choice | OrderedFloat (choice.score))
      .unwrap();
    
    vec![best_choice.choice.clone()]
  }
}

const MIN_SELECTION_PROBABILITY: f64 = 0.001;

fn logistic(x: f64)->f64 { 1.0/(1.0 + (-x).exp()) }

impl ChoiceWeights {
  fn get(&self, choice: &Choice)-> &[f64] {
    match choice {
      Choice::PlayCard (PlayCard {card, target}) => {
        &self.play_card_weights [card.card_info.id] [min (card.upgrades as usize, 1)]
      }
      Choice::EndTurn (_) => {
        &self.end_turn_weights
      }
      _=> panic!("NeuralStrategy doesn't yet know how to handle choices other than playing cards and ending turn"),
    }
  }
  fn get_mut (&mut self, choice: &Choice)-> &mut [f64] {
    match choice {
      Choice::PlayCard (PlayCard {card, target}) => {
        &mut self.play_card_weights [card.card_info.id] [min (card.upgrades as usize, 1)]
      }
      Choice::EndTurn (_) => {
        &mut self.end_turn_weights
      }
      _=> panic!("NeuralStrategy doesn't yet know how to handle choices other than playing cards and ending turn"),
    }
  }
}

impl NeuralStrategy {
  fn inputs (&self, state: & CombatState)->Vec<f64> {
    unimplemented!()
  }
  
  fn analyze (&self, state: & CombatState)->CombatStateAnalysis {
    let inputs = self.inputs (state);
    
    let mut hidden_inputs: Vec<f64> = iter::repeat (0.0).take (self.hidden_layer_size).collect();
    
    for (input_index, input) in inputs.iter().enumerate() {
      for (hidden_index, weight) in self.input_weights [input_index].iter().enumerate() {
        hidden_inputs [hidden_index] += input*weight;
      }
    }
    
    let hidden_outputs: Vec<f64> = hidden_inputs.iter().copied().map (logistic).collect();
    
    let legal_choices = state.legal_choices();
    let mut choices: Vec<_> = legal_choices.into_iter().map (| choice | {
      let score = self.choice_weights.get (&choice)
          .iter()
          .enumerate()
          .map(|(hidden_index, weight)| hidden_outputs [hidden_index] * weight)
          .sum::<f64>();
      
      ChoiceAnalysis {
        choice,
        score,
        selection_probability: MIN_SELECTION_PROBABILITY,
      }
    }).collect();
    
    choices.iter_mut().max_by_key (| choice | OrderedFloat (choice.score)).unwrap().selection_probability = 1.0 - ((choices.len() as f64 - 1.0)*MIN_SELECTION_PROBABILITY);
    
    CombatStateAnalysis {
      inputs,
      hidden_inputs,
      hidden_outputs,
      choices,
    }
  }
  
  fn backpropagate (&self, analysis: & CombatStateAnalysis, new_version: &mut Self, choice: & ChoiceAnalysis, observed_score: f64) {
    let learning_rate = MIN_SELECTION_PROBABILITY / choice.selection_probability;
    // desired_change_size = observed_score - choice.score;
    // error = desired_change_size^2
    // = choice.score^2 - 2*observed_score*choice.score + observed_score^2
    // derror/dscore = 2*choice.score - 2*observed_score
    // and the 2 is just a constant factor that we can adjust (since error is in arbitrary units) to apply the learning rate
    let derror_dscore = (choice.score - observed_score) * learning_rate;
    
    let old_weights = self.choice_weights.get (& choice.choice);
    let new_weights = new_version.choice_weights.get_mut (& choice.choice) ;
    
    for (hidden_index, hidden_output) in analysis.hidden_outputs.iter().enumerate() {
      let dscore_dweight = hidden_output;
      let derror_dweight = derror_dscore * dscore_dweight;
      new_weights[hidden_index] -= derror_dweight;
      
      let dscore_dhiddenoutput = old_weights [hidden_index];
      let derror_dhiddenoutput = derror_dscore * dscore_dhiddenoutput;
      
      // derivative assuming it was computed by the logistic function:
      let dhiddenoutput_dhiddeninput = hidden_output*(1.0-hidden_output);
      let derror_dhiddeninput = derror_dhiddenoutput * dhiddenoutput_dhiddeninput;
      for (input_index, input) in analysis.inputs.iter().enumerate() {
        let dscore_dweight = input;
        let derror_dweight = derror_dhiddeninput * dscore_dweight;
        new_version.input_weights [input_index][hidden_index] -= derror_dweight;
      }
    }
  }
}
