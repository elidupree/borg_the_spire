use crate::ai_utils::Strategy;
use crate::competing_optimizers::playout_result;
use crate::seed_system::SeedView;
use crate::simulation_state::CombatState;
use ordered_float::OrderedFloat;
use rand::seq::IteratorRandom;
use rand::Rng;

struct CandidateSubgroup<'a, T> {
  members: Vec<&'a T>,
  desirability: f64,
}

/**
Split a corpus into subgroups of size `subgroup_size`, optimizing the total `subgroup_desirability` among the subgroups.

The primary intent of this is to choose *representative samples*, where each individual has a score and each subgroup has a similar average score to other subgroups. If the `subgroup_desirability` function is a concave-downwards function (like -x^2) of the average score in the subgroup, this incentivizes the subgroups to have similar average scores.

The implementation is currently simulated annealing on random swaps. Some empirical tests I did a while back suggest that this is pretty close to optimal.

`subgroup_size` must be a divisor of the corpus size.
*/
pub fn representative_subgroups<'a, T>(
  corpus: &'a [T],
  subgroup_size: usize,
  subgroup_desirability: impl Fn(&[&T]) -> f64,
  rng: &mut impl Rng,
) -> Vec<Vec<&'a T>> {
  assert_ne!(subgroup_size, 0, "`subgroup_size` must be nonzero");
  assert_ne!(corpus.len(), 0, "`corpus` must be nonempty");
  let num_subgroups = corpus.len() / subgroup_size;
  assert_eq!(
    num_subgroups * subgroup_size,
    corpus.len(),
    "`subgroup_size` must be a divisor of the corpus size"
  );
  let mut subgroups: Vec<CandidateSubgroup<T>> = corpus
    .chunks_exact(subgroup_size)
    .map(|members| {
      let members: Vec<&T> = members.iter().collect();
      let desirability = subgroup_desirability(&members);
      CandidateSubgroup {
        members,
        desirability,
      }
    })
    .collect();
  assert_eq!(subgroups.len(), num_subgroups);
  let min_d = subgroups
    .iter()
    .map(|s| OrderedFloat(s.desirability))
    .min()
    .unwrap();
  let max_d = subgroups
    .iter()
    .map(|s| OrderedFloat(s.desirability))
    .max()
    .unwrap();
  let base_temperature = (max_d - min_d).0 * 0.25;
  let temperature_factors = [1.0, 0.7, 0.5, 0.2, 0.1, 0.0, 0.0, 0.0, 0.0];
  for &factor in &temperature_factors {
    let temperature = factor * base_temperature;
    for group1 in 0..num_subgroups {
      for member1 in 0..subgroup_size {
        let mut group2 = rng.gen_range(0..num_subgroups - 1);
        if group2 >= group1 {
          group2 += 1
        }
        let member2 = rng.gen_range(0..subgroup_size);

        let old_desirability = subgroups[group1].desirability + subgroups[group2].desirability;

        // can't use mem::swap if we can't hold the &mut's at the same time, alas
        let temp = subgroups[group2].members[member2];
        subgroups[group2].members[member2] = subgroups[group1].members[member1];
        subgroups[group1].members[member1] = temp;

        let new_desirability1 = subgroup_desirability(&subgroups[group1].members);
        let new_desirability2 = subgroup_desirability(&subgroups[group2].members);
        let new_desirability = new_desirability1 + new_desirability2;

        let acceptance_prob = if new_desirability > old_desirability {
          1.0
        } else if temperature == 0.0 {
          0.0
        } else {
          ((new_desirability - old_desirability) / temperature).exp()
        };
        //dbg!((old_desirability, new_desirability, acceptance_prob));
        if rng.gen::<f64>() < acceptance_prob {
          // confirm the swap: update the desirabilities
          subgroups[group1].desirability = new_desirability1;
          subgroups[group2].desirability = new_desirability2;
        } else {
          // cancel the swap: swap back
          let temp = subgroups[group2].members[member2];
          subgroups[group2].members[member2] = subgroups[group1].members[member1];
          subgroups[group1].members[member1] = temp;
        }
      }
    }
  }
  subgroups.into_iter().map(|s| s.members).collect()
}
pub fn representative_subgroup<'a, T>(
  corpus: &'a [T],
  subgroup_size: usize,
  subgroup_desirability: impl Fn(&[&T]) -> f64,
  rng: &mut impl Rng,
) -> Vec<&'a T> {
  representative_subgroups(corpus, subgroup_size, subgroup_desirability, rng)
    .into_iter()
    .choose(rng)
    .unwrap()
}

//exploiter_scores is indexed first by exploiting-strategy index and then by seed index
pub fn representative_seed_subgroup<T: SeedView<CombatState>>(
  corpus: &[&T],
  exploiter_scores: &[&[f64]],
  subgroup_size: usize,
  rng: &mut impl Rng,
) -> Vec<T> {
  let subgroup_desirability = |seeds: &[&usize]| {
    -exploiter_scores
      .iter()
      .map(|scores| seeds.iter().map(|&&seed| scores[seed]).sum::<f64>().powi(2))
      .sum::<f64>()
  };
  let corpus_indices: Vec<usize> = (0..corpus.len()).collect();
  let result_indices =
    representative_subgroup(&corpus_indices, subgroup_size, subgroup_desirability, rng);
  result_indices
    .into_iter()
    .map(|&index| corpus[index].clone())
    .collect()
}

struct RepresentativeSeedSearchLayerExploiter {
  hypothesized_average_score: f64,
  scores: Vec<f64>,
}

pub struct RepresentativeSeedSearchLayer<T> {
  seeds: Vec<T>,
  best_scores: Vec<f64>,
  exploiters: Vec<RepresentativeSeedSearchLayerExploiter>,
}

impl<T: SeedView<CombatState>> RepresentativeSeedSearchLayer<T> {
  pub fn new(seeds: Vec<T>, starting_state: &CombatState, strategy: &impl Strategy) -> Self {
    let scores = seeds
      .iter()
      .map(|seed| playout_result(starting_state, seed.clone(), strategy).score)
      .collect();

    RepresentativeSeedSearchLayer {
      seeds,
      best_scores: scores,
      exploiters: Vec::new(),
    }
  }
  /**
  Try a strategy which is hypothesized to be better than the current best.

  Typically, `strategy` will be a strategy that has been optimized on a subset of the current seeds, and performs better than the current best on the subset. This function evaluates it on the entire corpus, and checks whether it indeed performs better. If it does, we replace the current best with the new strategy; if it doesn't, we add it to our collection of "exploiters", and return a new subset that tries to resist the exploitation used by `strategy` as well as all previous exploiters.
  */
  pub fn try_strategy(
    &mut self,
    starting_state: &CombatState,
    strategy: &impl Strategy,
    hypothesized_average_score: f64,
    subgroup_size: usize,
    rng: &mut impl Rng,
  ) -> Result<(), Vec<T>> {
    let scores: Vec<f64> = self
      .seeds
      .iter()
      .map(|seed| playout_result(starting_state, seed.clone(), strategy).score)
      .collect();
    let sum = scores.iter().sum::<f64>();
    if sum > self.best_scores.iter().sum::<f64>() {
      let average = sum / self.seeds.len() as f64;
      self
        .exploiters
        .retain(|e| e.hypothesized_average_score > average);
      self.best_scores = scores;
      Ok(())
    } else {
      self
        .exploiters
        .push(RepresentativeSeedSearchLayerExploiter {
          hypothesized_average_score,
          scores,
        });
      let new_subgroup = representative_seed_subgroup(
        &self.seeds.iter().collect::<Vec<_>>(),
        &self
          .exploiters
          .iter()
          .map(|e| e.scores.as_slice())
          .collect::<Vec<_>>(),
        subgroup_size,
        rng,
      );
      Err(new_subgroup)
    }
  }
}
