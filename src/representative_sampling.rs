use crate::ai_utils::{playout_narration, playout_result, Strategy};
use crate::competing_optimizers::{ExplorationOptimizerKind, StrategyOptimizer};
use crate::seed_system::{Seed, SeedGenerator, SingleSeed, SingleSeedGenerator};
use crate::seeds_concrete::CombatChoiceLineagesKind;
use crate::simulation::Choice;
use crate::simulation_state::CombatState;
use ordered_float::OrderedFloat;
use rand::seq::{IteratorRandom, SliceRandom};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::Arc;

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
    "`subgroup_size` {} must be a divisor of the corpus size {}",
    subgroup_size,
    corpus.len()
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
pub fn representative_seed_subgroup(
  strategy_scores: &[&[f64]],
  subgroup_size: usize,
  rng: &mut impl Rng,
) -> Vec<usize> {
  let subgroup_desirability = |seeds: &[&usize]| {
    -strategy_scores
      .iter()
      .map(|scores| seeds.iter().map(|&&seed| scores[seed]).sum::<f64>().powi(2))
      .sum::<f64>()
  };
  let corpus_indices: Vec<usize> = (0..strategy_scores[0].len()).collect();
  representative_subgroup(&corpus_indices, subgroup_size, subgroup_desirability, rng)
    .into_iter()
    .copied()
    .collect()
}

/*
New fractal seed search design (not yet implemented):

New, improved strategies want to eventually be tested on a large corpus of seeds. However, there can be a long series of slightly-better strategies, and in this case, we don't want every strategy to be tested on every seed, because that could take too long. Instead, we stop testing at a lower seed-count until more strategies have had a chance to be tested to the same seed-count, and only pass along the best.

We would like there to be an amortized constant number of playouts for each submitted strategy. We achieve this through explicit credit tracking.

Fix a constant R. A strategy that has performed N playouts must also reserve N*R credits. When the strategy dies, the N*R credits are freed up to be spent on other strategies (although of course, it doesn't mean you can do N*R playouts, since each playout also requires reserving R credits - thus, releasing N*R credits means N*R/(R+1) more playouts can be done).

When a new strategy S is submitted, you proceed as follows: first you run a fixed number of playouts on S. Then, for K in 0..:
– you start with the assumption that all strategies have run at least 2^K playouts.
– you perform a culling process on those strategies, potentially killing some of them which have only 2^K playouts and seem worse than either their peers or other strategies with more playouts. This may release some credits into the "pool of spare credits at level K".
– you check whether there are enough spare credits at level K to raise every strategy with only 2^K playouts up to 2^(K+1) playouts. (maybe we can also spend credits from levels below K? not sure the difference is important. spending credits from higher levels would be wasteful though)
– – If yes, do it and continue the iteration.
– – If no, we aren't going to run more playouts right now. Since all strategies have run at least K playouts, now is a good time to resample all the representative seed collections at lower levels.

The culling process is something like:

A strategy survives if it meets *any* of the following:
– It has more than 2^K playouts. (It is an "established strategy"; Its eventual death will be handled at a higher level, not this one.)
– It is the best known strategy (highest average score) on the seeds on which it's been tested.
– It is among the best `max_exploiters` among known strategies according to the exploiter-quality function.

For example, a fresh strategy that does not outperform any established strategy in any particular way will die; a collection of up to `max_exploiters` fresh strategies that are meaningfully distinct from the established strategies and each other will survive without killing anything; a new exploiter added to that collection will either die or kill one of the existing exploiters.

Note that this process means there will be no more than (max_exploiters+1)*(highest value of K that has been reached) surviving strategies at any time.

A high value of `max_exploiters` slows down the process of reaching a high K – but I think that once you've reached a specific K with `max_exploiters` strategies, submitting *new* strategies is no slower than it would be with a lower value of `max_exploiters`.

The value of R affects how much time is spent at which levels - high R means more time is spent at higher levels compared with lower ones, and vice versa. (Naively, higher R also increases the total cost incurred by a fixed number of initial playouts for a strategy, but in principle you could also compensate by reducing the number of initial playouts.)

It feels a little nonintuitive that only the exact best strategy is kept, when there's a variable number of exploiters. But keeping e.g. "the best two" could easily just keep two identical strategies. We want some sort of combined metric, keeping strategies that are both distinctive and good - some slightly different formulation of the exploiter-quality function, maybe? (One where the strategy with the highest average always scores highest?)
 */

pub type StrategyId = usize;
pub struct FRSSLayer {
  pub spare_credits: f64,
  pub did_last_zero_strategy_ascension: bool,
}
pub struct FRSSStrategy<S> {
  pub strategy: Arc<S>,
  pub scores: Vec<f64>,
}
pub struct FRSSConfig<S> {
  pub min_level_to_leave_strategies_at: usize,
  pub reserved_credits_factor: f64,
  pub culling_func: Box<dyn Fn(&[FRSSStrategy<S>]) -> Vec<bool>>,
}
pub struct NewFractalRepresentativeSeedSearch<S, T, G> {
  pub layers: Vec<FRSSLayer>,
  pub seeds: Vec<T>,
  pub strategies: Vec<FRSSStrategy<S>>,
  pub seed_generator: G,
  pub starting_state: CombatState,
  pub config: FRSSConfig<S>,
}

impl<S> Default for FRSSConfig<S> {
  fn default() -> Self {
    // Note: Some quick tests in competing_optimizers suggested that the values of these
    // parameters can be changed by quite a bit in either direction without having much effect
    // on the quality of the system's output.
    FRSSConfig {
      min_level_to_leave_strategies_at: 5,
      reserved_credits_factor: 2.0,
      culling_func: Box::new(|strategies| cull_closest_to_dominated(strategies, 32)),
    }
  }
}

pub fn cull_closest_to_dominated<S>(
  strategies: &[FRSSStrategy<S>],
  max_survivors: usize,
) -> Vec<bool> {
  assert!(max_survivors >= 1);
  let mut num_survivors = strategies.len();
  let mut survivors: Vec<bool> = strategies.iter().map(|_| true).collect();
  if num_survivors <= max_survivors {
    return survivors;
  }

  // Make the "domination table".
  //
  // This is a score for each ordered pair (s1, s2) of distinct strategies.
  // The score is equal to the total of how much s1 outperforms s2
  // on seeds where s1 has a higher score than s2.
  //
  // Thus, if s1 is (non-strictly) dominated by s2, then s1 will have
  // a score of 0 against s2 and will be eliminated (unless another strategy
  // also scores 0 against somebody and gets eliminated first).
  //
  // If no strategy is (non-strictly) dominated, we'll cull the one that's *closest*
  // to dominated, as defined by having the *minimum* among the scores in the table.
  //
  // Note that a strategy  `best` with the highest average score will never be eliminated,
  // because `domination(best, s2) > domination(s2, best)` for all s2,
  // so s2 would be eliminated first.
  let mut domination_table: Vec<Vec<f64>> = strategies
    .iter()
    .map(|_| strategies.iter().map(|_| 0.0).collect())
    .collect();
  let comparable_size = strategies.iter().map(|s| s.scores.len()).min().unwrap();
  for (i1, s1) in strategies.iter().enumerate() {
    for (i2, s2) in strategies.iter().enumerate().skip(i1 + 1) {
      for seed_index in 0..comparable_size {
        let p1 = s1.scores[seed_index];
        let p2 = s2.scores[seed_index];
        if p1 > p2 {
          domination_table[i1][i2] += p1 - p2;
        } else {
          domination_table[i2][i1] += p2 - p1;
        }
      }
    }
  }

  // Note that we can't just cull the N most dominated at once -
  // we have to do a loop, because otherwise two identical strategies would kill each other
  // simultaneously, even if they're the best. Instead, we want ONE to die,
  // after which the other survives because it's no longer (non-strictly) dominated by the other.
  while num_survivors > max_survivors {
    let most_dominated = strategies
      .iter()
      .zip(&domination_table)
      .enumerate()
      .filter(|&(i1, _)| survivors[i1])
      .min_by_key(|&(i1, (s1, row))| {
        let lowest_domination_by_s1 = row
          .iter()
          .enumerate()
          .filter(|&(i2, _)| i2 != i1 && survivors[i2])
          .map(|(_i2, &domination)| OrderedFloat(domination))
          .min()
          .unwrap();
        // include number of playouts as a tiebreaker (having more playouts makes you better,
        // in cases like "two identical strategies are non-strictly dominating each other"
        (lowest_domination_by_s1, s1.scores.len())
      })
      .unwrap()
      .0;
    assert!(survivors[most_dominated]);
    survivors[most_dominated] = false;
    num_survivors -= 1;
  }

  // let averages: Vec<_> = strategies
  //   .iter()
  //   .map(|s| OrderedFloat(s.scores[..comparable_size].iter().sum::<f64>()))
  //   .collect();
  // let best = averages.iter().max().unwrap();
  //dbg!(domination_table);
  // assert!(averages
  //   .iter()
  //   .zip(&survivors)
  //   .any(|(a, &survived)| a == best && survived));

  survivors
}

impl<S: Strategy + 'static, T: Seed<CombatState> + 'static, G: SeedGenerator<T> + 'static>
  NewFractalRepresentativeSeedSearch<S, T, G>
{
  pub fn new(starting_state: CombatState, seed_generator: G, config: FRSSConfig<S>) -> Self {
    NewFractalRepresentativeSeedSearch {
      layers: Vec::new(),
      seeds: Vec::new(),
      strategies: Vec::new(),
      seed_generator,
      starting_state,
      config,
    }
  }

  /// We expect to run at least a certain number of playouts for every generated strategy, to make sure it is bad rather than merely unlucky before we eliminated. Thus, if you are generating a large number of strategies that are extremely cheap to generate, it's best to cull the ones that are not promising before even submitting them to the FractalRepresentativeSeedSearch.
  ///
  pub fn consider_strategy(
    &mut self,
    strategy: Arc<S>,
    min_playouts_before_culling: usize,
    rng: &mut impl Rng,
  ) {
    assert!(min_playouts_before_culling <= 1 << self.config.min_level_to_leave_strategies_at);
    self.strategies.push(FRSSStrategy {
      strategy,
      scores: Vec::new(),
    });
    for level in 0.. {
      let level_size = 1 << level;
      if level == self.layers.len() {
        self.layers.push(FRSSLayer {
          spare_credits: 0.0,
          did_last_zero_strategy_ascension: false,
        });
        let seed_generator = &mut self.seed_generator;
        self
          .seeds
          .extend((self.seeds.len()..level_size).map(|_| seed_generator.make_seed()));
      }
      assert!(self.layers.len() > level);
      assert!(self.seeds.len() >= level_size);
      for strategy in &mut self.strategies {
        if strategy.scores.len() < level_size {
          for seed in &self.seeds[strategy.scores.len()..level_size] {
            strategy
              .scores
              .push(playout_result(&self.starting_state, seed.view(), &*strategy.strategy).score)
          }
        }
        assert!(strategy.scores.len() >= level_size);
      }

      let survivors = (self.config.culling_func)(&self.strategies);
      let mut index = 0;
      let last_index = self.strategies.len() - 1;
      let config = &self.config;
      let layer = self.layers.get_mut(level).unwrap();
      self.strategies.retain(|strategy| {
        let result = survivors[index]
          || strategy.scores.len() > level_size
            // up to min_playouts_before_culling, the just-submitted strategy will always be the last:
          || (level_size < min_playouts_before_culling && index == last_index);
        if !result {
          layer.spare_credits += strategy.scores.len() as f64 * config.reserved_credits_factor
        }
        index += 1;
        result
      });

      // The rest of this loop is about deciding whether to continue to a larger layer.
      //
      // There are various different conditions where you would want to ascend to handling larger layers. For each condition, we either `continue` to proceed to the next layer, or fall through, reaching the `break` at the end of the loop if none of the continue-conditions are met.
      if level < self.config.min_level_to_leave_strategies_at {
        continue;
      }

      let next_level_size = 1usize << (level + 1);
      let steps_needed_to_advance = self
        .strategies
        .iter()
        .map(|s| next_level_size.saturating_sub(s.scores.len()))
        .sum::<usize>();
      if steps_needed_to_advance == 0 {
        // everyone died, which means we TECHNICALLY have enough credits to advance! Our credits can still be used at higher levels, and it's still useful to do resampling from higher levels. However, if every strategy is dying at a low level, and you ascend as far as you can every time, then the resampling ends up having a performance cost comparable to the playouts. I verified this using profiling data. So, in this case, each level only ascends HALF the time, meaning the total amortized cost is more like O(log (biggest level size) than O(biggest level size).
        if self.layers[level].did_last_zero_strategy_ascension {
          self.layers[level].did_last_zero_strategy_ascension = false;
        } else {
          self.layers[level].did_last_zero_strategy_ascension = true;
          continue;
        }
      } else {
        let credits_needed_to_advance =
          steps_needed_to_advance as f64 * (1.0 + self.config.reserved_credits_factor);
        let credits_available = self.layers[..=level]
          .iter()
          .map(|l| l.spare_credits)
          .sum::<f64>();
        if credits_available >= credits_needed_to_advance {
          let mut credits_left_to_drain = credits_needed_to_advance;
          for layer in self.layers[..=level].iter_mut().rev() {
            if layer.spare_credits > credits_left_to_drain {
              layer.spare_credits -= credits_left_to_drain;
              break;
            } else {
              credits_left_to_drain -= layer.spare_credits;
              layer.spare_credits = 0.0;
            }
          }
          continue;
        }
      }

      // fell through - not able to advance
      self.resample_below_level(level, rng);
      break;
    }
  }

  pub fn resample_below_level(&mut self, level: usize, rng: &mut impl Rng) {
    for strategy in &self.strategies {
      assert!(strategy.scores.len() >= 1 << level);
    }
    for lower_level in (0..level).rev() {
      let subgroup_size = 1 << lower_level;
      let mut moving_indices = representative_seed_subgroup(
        &self
          .strategies
          .iter()
          .map(|e| &e.scores[..1 << (lower_level + 1)])
          .collect::<Vec<_>>(),
        subgroup_size,
        rng,
      );
      let can_stay_still_indices: HashSet<usize> = moving_indices
        .drain_filter(|&mut i| i < subgroup_size)
        .collect();
      let mut must_move_indices = moving_indices.into_iter();
      for target_index in 0..subgroup_size {
        if can_stay_still_indices.contains(&target_index) {
          // this seed is already in a good location, do nothing
        } else {
          let moving_index = must_move_indices.next().unwrap();

          // because strategy scores are in the same order as seeds, we must reorder them too
          self.seeds.swap(moving_index, target_index);
          for strategy in &mut self.strategies {
            strategy.scores.swap(moving_index, target_index)
          }
        }
      }
      assert!(must_move_indices.next().is_none());
    }
  }

  pub fn meta_strategy(&self) -> RepresentativeSeedsMetaStrategy<S, T> {
    let mut strategies: Vec<&_> = self.strategies.iter().collect();
    strategies.sort_by_key(|s| (s.scores.len(), OrderedFloat(s.scores.iter().sum::<f64>())));
    RepresentativeSeedsMetaStrategy {
      seeds: self.seeds.iter().take(16).cloned().collect(),
      strategies: strategies
        .iter()
        .rev()
        .take(16)
        .map(|s| s.strategy.clone())
        .collect(),
    }
  }

  pub fn report(&self) -> Arc<RepresentativeSeedsMetaStrategy<S, T>> {
    //   let best = &self
    //     .strategies
    //     .iter()
    //     .max_by_key(|s| (s.scores.len(), OrderedFloat(s.scores.iter().sum::<f64>())))
    //     .unwrap();

    println!("NewFractalRepresentativeSeedSearch reporting:",);
    for level in 0.. {
      let level_size = 1 << level;
      if level_size > self.seeds.len() {
        break;
      }
      let mut here_strategies: Vec<_> = self
        .strategies
        .iter()
        .filter(|s| s.scores.len() == level_size)
        .collect();
      here_strategies.sort_by_key(|s| OrderedFloat(-s.scores.iter().sum::<f64>()));
      let scores = here_strategies
        .iter()
        .map(|s| {
          format!(
            "{:.3}",
            s.scores.iter().sum::<f64>() / s.scores.len() as f64
          )
        })
        .collect::<Vec<_>>();
      let score_with_exploiting = (0..level_size)
        .map(|index| {
          self
            .strategies
            .iter()
            .filter_map(|s| s.scores.get(index))
            .max_by_key(|&&f| OrderedFloat(f))
            .unwrap()
        })
        .sum::<f64>()
        / (level_size as f64);
      let best_average = self
        .strategies
        .iter()
        .filter(|s| s.scores.len() >= level_size)
        .map(|s| s.scores[..level_size].iter().sum::<f64>())
        .max_by_key(|&f| OrderedFloat(f))
        .unwrap()
        / (level_size as f64);
      println!(
        "{:>5}: [{:.3} > {:.3}] ({:.1}) {}",
        level_size,
        score_with_exploiting,
        best_average,
        self.layers[level].spare_credits,
        scores.join(", ")
      );
    }

    //&best.strategy

    Arc::new(self.meta_strategy())
  }
}

#[derive(Clone)]
pub struct RepresentativeSeedSearchLayerStrategy<S> {
  pub strategy: Arc<S>,
  pub scores: Vec<f64>,
  pub average: f64,
}

impl<S: Strategy> RepresentativeSeedSearchLayerStrategy<S> {
  fn new(seeds: &[impl Seed<CombatState>], strategy: Arc<S>, starting_state: &CombatState) -> Self {
    let scores: Vec<f64> = seeds
      .iter()
      .map(|seed| playout_result(starting_state, seed.view(), &*strategy).score)
      .collect();
    let average = scores.iter().sum::<f64>() / scores.len() as f64;
    RepresentativeSeedSearchLayerStrategy {
      strategy,
      scores,
      average,
    }
  }
  fn new_with_scores(strategy: Arc<S>, scores: Vec<f64>) -> Self {
    let average = scores.iter().sum::<f64>() / scores.len() as f64;
    RepresentativeSeedSearchLayerStrategy {
      strategy,
      scores,
      average,
    }
  }
}

pub struct RepresentativeSeedSearchLayer<S, T> {
  pub seeds: Vec<T>,
  pub max_exploiters: usize,
  pub best_strategy: Arc<RepresentativeSeedSearchLayerStrategy<S>>,
  pub exploiters: Vec<Arc<RepresentativeSeedSearchLayerStrategy<S>>>,
}

impl<S: Strategy, T: Seed<CombatState>> RepresentativeSeedSearchLayer<S, T> {
  pub fn new(
    seeds: Vec<T>,
    starting_state: &CombatState,
    strategy: Arc<S>,
    max_exploiters: usize,
  ) -> Self {
    let best_strategy = Arc::new(RepresentativeSeedSearchLayerStrategy::new(
      &seeds,
      strategy,
      starting_state,
    ));
    let exploiters = vec![best_strategy.clone()];
    RepresentativeSeedSearchLayer {
      seeds,
      max_exploiters,
      best_strategy,
      exploiters,
    }
  }
  pub fn new_with_precalculated_strategies(
    seeds: Vec<T>,
    strategies: Vec<Arc<RepresentativeSeedSearchLayerStrategy<S>>>,
    max_exploiters: usize,
  ) -> Self {
    let best_strategy = strategies
      .iter()
      .max_by_key(|s| OrderedFloat(s.average))
      .unwrap()
      .clone();
    let mut result = RepresentativeSeedSearchLayer {
      seeds,
      max_exploiters,
      best_strategy,
      exploiters: strategies,
    };
    result.drop_excess_exploiters();
    result
  }
  pub fn strategies(&self) -> impl Iterator<Item = &Arc<RepresentativeSeedSearchLayerStrategy<S>>> {
    std::iter::once(&self.best_strategy).chain(
      self
        .exploiters
        .iter()
        .filter(move |e| !Arc::ptr_eq(e, &self.best_strategy)),
    )
  }
  fn drop_excess_exploiters(&mut self) {
    while self.exploiters.len() > self.max_exploiters {
      // On each seed, each strategy grants credit to all strategies that are better than it; the strategy with the least credit at the end is dropped. Theoretically, for each strategy `S` and amount `p > 0.0` there's a fixed infinitesimal reward for "being better than strategy S by at least `p` points", which is shared evenly among all strategies that are at least `p` points better than S.
      let mut total_credit: Vec<_> = self.exploiters.iter().map(|_| 0.0).collect();
      for seed_index in 0..self.seeds.len() {
        let mut scores: Vec<_> = self
          .exploiters
          .iter()
          .map(|e| e.scores[seed_index])
          .enumerate()
          .collect();
        scores.sort_by_key(|&(_, s)| OrderedFloat(s));
        let (&(_, mut previous_score), rest) = scores.split_first().unwrap();
        let mut remaining_creditors = rest.len();
        let mut credit_to_remaining_creditors = 0.0;
        for &(exploiter_index, score) in rest {
          let difference = score - previous_score;
          credit_to_remaining_creditors += difference / remaining_creditors as f64;
          total_credit[exploiter_index] += credit_to_remaining_creditors;
          remaining_creditors -= 1;
          previous_score = score;
        }
      }
      let worst_index = total_credit
        .into_iter()
        .enumerate()
        .min_by_key(|&(_, c)| OrderedFloat(c))
        .unwrap()
        .0;
      self.exploiters.remove(worst_index);
    }
  }

  /**
  Submit a strategy for consideration as either the new best or a useful exploiter.

  Typically, `strategy` will be a strategy that has been optimized on a subset of the current seeds, and performs better than the current best on the subset. This function evaluates it on the entire corpus, and checks whether it indeed performs better.
  */
  pub fn consider_strategy(&mut self, starting_state: &CombatState, strategy: Arc<S>) {
    let strategy = Arc::new(RepresentativeSeedSearchLayerStrategy::new(
      &self.seeds,
      strategy,
      starting_state,
    ));
    self.exploiters.push(strategy.clone());
    self.drop_excess_exploiters();
    if strategy.average > self.best_strategy.average {
      self.best_strategy = strategy;
    }
  }
  pub fn make_sublayer(&self, subgroup_size: usize, rng: &mut impl Rng) -> Self {
    let sublayer_indices = representative_seed_subgroup(
      &self
        .strategies()
        .map(|e| e.scores.as_slice())
        .collect::<Vec<_>>(),
      subgroup_size,
      rng,
    );
    let sublayer_seeds: Vec<T> = sublayer_indices
      .iter()
      .map(|&index| self.seeds[index].clone())
      .collect();
    let sublayer_strategies: Vec<_> = self
      .strategies()
      .map(|strategy| {
        Arc::new(RepresentativeSeedSearchLayerStrategy::new_with_scores(
          strategy.strategy.clone(),
          sublayer_indices
            .iter()
            .map(|&index| strategy.scores[index])
            .collect(),
        ))
      })
      .collect();
    for (strategy, sublayer_strategy) in self.strategies().zip(&sublayer_strategies) {
      let a = strategy.average;
      let b = sublayer_strategy.average;
      let unavoidable_difference =
        (((a * subgroup_size as f64).round() / subgroup_size as f64) - a).abs();
      if (a - b).abs() > unavoidable_difference + (0.2 / subgroup_size as f64) {
        // println!(
        //   "A strategy's average score had an unfortunately large difference in the subgroup: {}: {} -> {} ({})",
        //   self.seeds.len(),
        //   a,
        //   b,
        //   unavoidable_difference
        // )
      }
    }
    Self::new_with_precalculated_strategies(
      sublayer_seeds,
      sublayer_strategies,
      self.max_exploiters,
    )
  }
  pub fn make_superlayer(
    &self,
    supergroup_size: usize,
    mut make_seed: impl FnMut() -> T,
    starting_state: &CombatState,
  ) -> Self {
    let mut superlayer_seeds = self.seeds.clone();
    let extras = supergroup_size - superlayer_seeds.len();
    superlayer_seeds.extend((0..extras).map(|_| make_seed()));
    assert_eq!(superlayer_seeds.len(), supergroup_size);
    let superlayer_strategies: Vec<_> = self
      .strategies()
      .map(|strategy| {
        Arc::new(RepresentativeSeedSearchLayerStrategy::new(
          &superlayer_seeds,
          strategy.strategy.clone(),
          starting_state,
        ))
      })
      .collect();
    Self::new_with_precalculated_strategies(
      superlayer_seeds,
      superlayer_strategies,
      self.max_exploiters,
    )
  }
}

pub struct FractalRepresentativeSeedSearch<S, T, G> {
  pub layers: Vec<RepresentativeSeedSearchLayer<S, T>>,
  pub lowest_seeds: Vec<T>,
  pub seed_generator: G,
  pub best_average_on_lowest_seeds: f64,
  pub starting_state: CombatState,
  pub steps: usize,
  pub successes_at_lowest: usize,
  pub layer_updates: usize,
}
impl<S, T, G> FractalRepresentativeSeedSearch<S, T, G> {
  pub fn sublayer_size(index: usize) -> usize {
    4 << index
  }
  pub fn layer_size(index: usize) -> usize {
    Self::sublayer_size(index + 1)
  }
}

impl<S: Strategy + 'static, T: Seed<CombatState> + 'static, G: SeedGenerator<T> + 'static>
  FractalRepresentativeSeedSearch<S, T, G>
{
  pub fn new(starting_state: CombatState, mut seed_generator: G, first_strategy: Arc<S>) -> Self {
    let seeds = (0..Self::layer_size(0))
      .map(|_| seed_generator.make_seed())
      .collect();
    let first_layer =
      RepresentativeSeedSearchLayer::new(seeds, &starting_state, first_strategy, 16);
    let below_first_layer =
      first_layer.make_sublayer(Self::sublayer_size(0), &mut rand::thread_rng());
    let lowest_seeds = below_first_layer.seeds;
    let best_average_on_lowest_seeds = below_first_layer.best_strategy.average;
    FractalRepresentativeSeedSearch {
      layers: vec![first_layer],
      lowest_seeds,
      seed_generator,
      best_average_on_lowest_seeds,
      starting_state,
      steps: 0,
      successes_at_lowest: 0,
      layer_updates: 0,
    }
  }

  pub fn meta_strategy(&self) -> RepresentativeSeedsMetaStrategy<S, T> {
    RepresentativeSeedsMetaStrategy {
      seeds: self
        .layers
        .get(1)
        .unwrap_or_else(|| self.layers.last().unwrap())
        .seeds
        .clone(),
      strategies: self
        .layers
        .last()
        .unwrap()
        .strategies()
        .map(|s| s.strategy.clone())
        .collect(),
    }
  }

  pub fn consider_strategy(&mut self, strategy: Arc<S>) {
    self.steps += 1;
    self.lowest_seeds.shuffle(&mut rand::thread_rng());
    let mut total_score = 0.0;
    let mut average = -999999999999.0;
    for (index, seed) in self.lowest_seeds.iter().enumerate() {
      let result = playout_result(&self.starting_state, seed.view(), &*strategy);
      total_score += result.score;
      average = total_score / (index + 1) as f64;
      if average < self.best_average_on_lowest_seeds {
        break;
      }
    }
    if average >= self.best_average_on_lowest_seeds {
      self.successes_at_lowest += 1;
      self.layers[0].consider_strategy(&self.starting_state, strategy)
    }
    // if there's no new strategy to consider, generally don't reroll the layers,
    // but occasionally we could have a nonrepresentative smallest layer with a pathologically good score,
    // so occasionally reroll them anyway
    else if self.steps % 64 != 0 {
      return;
    }
    self.layer_updates += 1;

    // Each layer is twice as big as the last, so it is twice as much work to try strategies on it. Thus, visit each layer only one-third as often as the last, keeping the total amortized cost only as great as that of the lowest layer.
    let mut steps_thingy = self.layer_updates;
    let mut layer_to_resample_index: usize = 0;
    while steps_thingy % 3 == 0 {
      layer_to_resample_index += 1;
      steps_thingy /= 3;
    }
    for index in 0..(self.layers.len() - 1).min(layer_to_resample_index) {
      for strategy in self.layers[index].strategies().cloned().collect::<Vec<_>>() {
        self.layers[index + 1].consider_strategy(&self.starting_state, strategy.strategy.clone());
      }
    }
    if layer_to_resample_index >= self.layers.len() {
      assert_eq!(layer_to_resample_index, self.layers.len());
      let last = self.layers.last().unwrap();
      assert_eq!(
        last.seeds.len(),
        Self::sublayer_size(layer_to_resample_index)
      );
      let seed_generator = &mut self.seed_generator;
      let new_layer = last.make_superlayer(
        Self::layer_size(layer_to_resample_index),
        || seed_generator.make_seed(),
        &self.starting_state,
      );
      self.layers.push(new_layer);
    }
    for index in (0..layer_to_resample_index).rev() {
      let new_sublayer =
        self.layers[index + 1].make_sublayer(Self::layer_size(index), &mut rand::thread_rng());
      self.layers[index] = new_sublayer;
    }
    let new_sublayer =
      self.layers[0].make_sublayer(Self::sublayer_size(0), &mut rand::thread_rng());
    self.lowest_seeds = new_sublayer.seeds;
    self.best_average_on_lowest_seeds = new_sublayer.best_strategy.average;
  }

  pub fn report(&self) -> Arc<RepresentativeSeedsMetaStrategy<S, T>> {
    let best = &self.layers.last().unwrap().best_strategy;

    println!(
      "FractalRepresentativeSeedSearch reporting strategy with average score of {} ({}/{}/{} steps, {} layers, max {} seeds)",
      best.average, self.steps, self.layer_updates, self.successes_at_lowest, self.layers.len(), self.layers.last().unwrap().seeds.len()
    );
    for layer in &self.layers {
      let strategies: Vec<_> = layer.strategies().collect();
      let scores = strategies
        .iter()
        .map(|s| format!("{:.3}", s.average))
        .collect::<Vec<_>>();
      let score_with_exploiting = (0..layer.seeds.len())
        .map(|index| {
          strategies
            .iter()
            .map(|s| s.scores[index])
            .max_by_key(|&f| OrderedFloat(f))
            .unwrap()
        })
        .sum::<f64>()
        / (layer.seeds.len() as f64);
      println!(
        "{}: [{:.3}] {}",
        layer.seeds.len(),
        score_with_exploiting,
        scores.join(", ")
      );
    }

    //&best.strategy

    Arc::new(self.meta_strategy())
  }

  fn diagnose_exploitations(&self) {
    let meta_strategy = self.meta_strategy();
    let last = self.layers.last().unwrap();
    for (index, seed) in last.seeds.iter().take(1000).enumerate() {
      let best_exploiter = last
        .strategies()
        .max_by_key(|s| OrderedFloat(s.scores[index]))
        .unwrap();
      if best_exploiter.scores[index] > 0.5 {
        if playout_result(&self.starting_state, seed.view(), &meta_strategy).score < 0.5 {
          let meta_narration = playout_narration(&self.starting_state, seed.view(), &meta_strategy);
          let exploiter_narration =
            playout_narration(&self.starting_state, seed.view(), &*best_exploiter.strategy);
          let diff = difference::Changeset::new(&meta_narration, &exploiter_narration, "\n");
          println!("\n===== Caught meta-strategy playing worse than exploiter =====");
          println!("=== Meta-strategy playout: ===");
          println!("{}", meta_narration);
          println!("=== Best exploiter playout: ===");
          println!("{}", exploiter_narration);
          println!("=== Diff: ===");
          println!("{}", diff);
          println!("===== End of meta-strategy playing worse than exploiter =====\n");
        }
      }
    }
  }
}

#[derive(Clone, Debug)]
pub struct RepresentativeSeedsMetaStrategy<S, T> {
  pub seeds: Vec<T>,
  pub strategies: Vec<Arc<S>>,
}

impl<S: Strategy + 'static, T: Seed<CombatState> + Clone + 'static> Strategy
  for RepresentativeSeedsMetaStrategy<S, T>
{
  fn choose_choice(&self, state: &CombatState) -> Vec<Choice> {
    let best_strategy = self
      .strategies
      .iter()
      .max_by_key(|&strategy| {
        let score = self
          .seeds
          .iter()
          .map(|seed| playout_result(&state, seed.view(), &**strategy).score)
          .sum::<f64>();
        OrderedFloat(score)
      })
      .unwrap();
    best_strategy.choose_choice(state)
  }
}

pub struct FractalRepresentativeSeedSearchOptimizer<S> {
  pub seed_search:
    FractalRepresentativeSeedSearch<S, SingleSeed<CombatChoiceLineagesKind>, SingleSeedGenerator>,
  pub new_strategy: Box<dyn Fn(&[&S]) -> S>,
}
pub struct FractalRepresentativeSeedSearchExplorationOptimizerKind;

impl<S: Strategy + 'static> StrategyOptimizer for FractalRepresentativeSeedSearchOptimizer<S> {
  type Strategy = RepresentativeSeedsMetaStrategy<S, SingleSeed<CombatChoiceLineagesKind>>;
  fn step(&mut self, _state: &CombatState, _rng: &mut ChaCha8Rng) {
    self
      .seed_search
      .consider_strategy(Arc::new((self.new_strategy)(
        &self
          .seed_search
          .layers
          .last()
          .unwrap()
          .strategies()
          .map(|s| &*s.strategy)
          .collect::<Vec<_>>(),
      )));
  }

  fn print_extra_info(&self, _state: &CombatState) {
    self.seed_search.diagnose_exploitations();
  }

  fn report(&self) -> Arc<Self::Strategy> {
    self.seed_search.report()
  }
}
impl ExplorationOptimizerKind for FractalRepresentativeSeedSearchExplorationOptimizerKind {
  type ExplorationOptimizer<T: Strategy + 'static> = FractalRepresentativeSeedSearchOptimizer<T>;

  fn new<T: Strategy + 'static>(
    self,
    starting_state: &CombatState,
    rng: &mut ChaCha8Rng,
    new_strategy: Box<dyn Fn(&[&T]) -> T>,
  ) -> Self::ExplorationOptimizer<T> {
    FractalRepresentativeSeedSearchOptimizer {
      seed_search: FractalRepresentativeSeedSearch::<T, _, _>::new(
        starting_state.clone(),
        SingleSeedGenerator::new(ChaCha8Rng::from_rng(rng).unwrap()),
        Arc::new(new_strategy(&[])),
      ),
      new_strategy,
    }
  }
}

pub struct NewFractalRepresentativeSeedSearchOptimizer<S> {
  pub seed_search: NewFractalRepresentativeSeedSearch<
    S,
    SingleSeed<CombatChoiceLineagesKind>,
    SingleSeedGenerator,
  >,
  pub new_strategy: Box<dyn Fn(&[&S]) -> S>,
}
#[derive(Copy, Clone, Serialize, Deserialize, Debug)]
pub struct NewFractalRepresentativeSeedSearchExplorationOptimizerKind {
  pub min_level_to_leave_strategies_at: usize,
  pub reserved_credits_factor: f64,
  pub max_survivors: usize,
}

impl<S: Strategy + 'static> StrategyOptimizer for NewFractalRepresentativeSeedSearchOptimizer<S> {
  type Strategy = RepresentativeSeedsMetaStrategy<S, SingleSeed<CombatChoiceLineagesKind>>;
  fn step(&mut self, _state: &CombatState, rng: &mut ChaCha8Rng) {
    self.seed_search.consider_strategy(
      Arc::new((self.new_strategy)(
        &self
          .seed_search
          .strategies
          .iter()
          .map(|s| &*s.strategy)
          .collect::<Vec<_>>(),
      )),
      0,
      rng,
    );
  }

  fn print_extra_info(&self, _state: &CombatState) {
    //self.seed_search.diagnose_exploitations();
  }

  fn report(&self) -> Arc<Self::Strategy> {
    self.seed_search.report()
  }
}
impl ExplorationOptimizerKind for NewFractalRepresentativeSeedSearchExplorationOptimizerKind {
  type ExplorationOptimizer<T: Strategy + 'static> = NewFractalRepresentativeSeedSearchOptimizer<T>;

  fn new<T: Strategy + 'static>(
    self,
    starting_state: &CombatState,
    rng: &mut ChaCha8Rng,
    new_strategy: Box<dyn Fn(&[&T]) -> T>,
  ) -> Self::ExplorationOptimizer<T> {
    NewFractalRepresentativeSeedSearchOptimizer {
      seed_search: NewFractalRepresentativeSeedSearch::<T, _, _>::new(
        starting_state.clone(),
        SingleSeedGenerator::new(ChaCha8Rng::from_rng(rng).unwrap()),
        FRSSConfig {
          min_level_to_leave_strategies_at: self.min_level_to_leave_strategies_at,
          reserved_credits_factor: self.reserved_credits_factor,
          culling_func: Box::new(move |strategies| {
            cull_closest_to_dominated(strategies, self.max_survivors)
          }),
        },
      ),
      new_strategy,
    }
  }
}
