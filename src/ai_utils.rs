use crate::actions::{DynAction, PlayCard};
use crate::seed_system::{NoRandomness, SeedView};
use crate::simulation::{
  Action, Choice, ConsiderAction, CreatureIndex, Runner, StandardRunner, StandardRunnerHooks,
};
use crate::simulation_state::cards::consider_card_actions;
use crate::simulation_state::{CombatState, PowerId, SingleCard, MAX_MONSTERS};
use arrayvec::ArrayVec;
use std::collections::{HashSet, VecDeque};
use std::fmt;
use std::fmt::{Debug, Write};

pub trait Strategy: Debug {
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
        let mut runner = StandardRunner::new(&mut new_state, NoRandomness);
        runner.apply_choice(&choice);
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

// I always want my profiling to show what's in playouts:
#[inline(never)]
pub fn play_out<S: Strategy>(runner: &mut impl Runner, strategy: &S) {
  runner.run_until_unable();
  while !runner.state().combat_over() {
    let choices = strategy.choose_choice(runner.state());
    for choice in choices {
      runner.apply_choice(&choice);
    }
  }
}

pub fn playout_result(
  state: &CombatState,
  seed: impl SeedView<CombatState>,
  strategy: &impl Strategy,
) -> CombatResult {
  let mut state = state.clone();
  play_out(&mut StandardRunner::new(&mut state, seed), strategy);
  CombatResult::new(&state)
}

pub struct NarrationHooks<'a, W: fmt::Write> {
  writer: &'a mut W,
  last_hand: ArrayVec<SingleCard, 10>,
  last_hitpoints: i32,
}
impl<'a, W: fmt::Write> NarrationHooks<'a, W> {
  fn write_combatants(&mut self, state: &CombatState) {
    write!(self.writer, "{} vs. ", state.player).unwrap();
    for monster in &state.monsters {
      write!(self.writer, "{}, ", monster).unwrap();
    }
    writeln!(self.writer).unwrap();
  }
  fn write_hand(&mut self, state: &CombatState) {
    write!(self.writer, "[{}] [", state.draw_pile.len()).unwrap();
    for card in &state.hand {
      write!(self.writer, "{}, ", card).unwrap();
    }
    writeln!(self.writer, "] [{}]", state.discard_pile.len()).unwrap();
  }
}
impl<'a, W: fmt::Write> StandardRunnerHooks for NarrationHooks<'a, W> {
  fn on_choice(&mut self, state: &CombatState, choice: &Choice) {
    if state.player.creature.hitpoints != self.last_hitpoints {
      writeln!(
        self.writer,
        "Took {} damage",
        self.last_hitpoints - state.player.creature.hitpoints
      )
      .unwrap();
      self.last_hitpoints = state.player.creature.hitpoints;
    }

    if state.hand != self.last_hand {
      self.last_hand = state.hand.clone();
      self.write_hand(state);
    }
    match choice {
      Choice::PlayCard(PlayCard { card, target }) => {
        let card_index = self.last_hand.iter().position(|c| c == card).unwrap();
        self.last_hand.remove(card_index);
        if card.card_info.has_target {
          writeln!(self.writer, "{} {}", card, target).unwrap();
        } else {
          writeln!(self.writer, "{}", card).unwrap();
        }
      }
      Choice::EndTurn(_) => {
        writeln!(self.writer, "=== EndTurn ===").unwrap();
        self.write_combatants(state);
      }
      _ => {}
    }
  }
  fn on_action(&mut self, state: &CombatState, action: &DynAction) {
    match action {
      DynAction::EndMonstersTurns(_) => {
        self.write_combatants(state);
      }
      _ => {}
    }
  }
}
pub fn playout_narration(
  state: &CombatState,
  seed: impl SeedView<CombatState>,
  strategy: &impl Strategy,
) -> String {
  let mut state = state.clone();
  let mut writer = String::new();
  let mut hooks = NarrationHooks {
    writer: &mut writer,
    last_hand: state.hand.clone(),
    last_hitpoints: state.player.creature.hitpoints,
  };
  hooks.write_combatants(&state);
  hooks.write_hand(&state);
  play_out(
    &mut StandardRunner::new(&mut state, seed).with_hooks(&mut hooks),
    strategy,
  );

  writeln!(hooks.writer, "Combat over.").unwrap();
  hooks.write_combatants(&state);

  writer
}

pub fn starting_choices_made_by_strategy(
  state: &CombatState,
  strategy: &impl Strategy,
) -> Vec<Choice> {
  let mut state = state.clone();
  let mut runner = StandardRunner::new(&mut state, NoRandomness);
  let mut result = Vec::new();
  while runner.state().choice_next() {
    let choices = strategy.choose_choice(runner.state());
    for choice in choices {
      runner.apply_choice(&choice);
      result.push(choice);
    }
  }
  result
}

#[derive(Debug, Default)]
pub struct CardPlayStats {
  pub block_amount: i32,
  pub damage: ArrayVec<f64, MAX_MONSTERS>,
}
pub fn card_play_stats(state: &CombatState, card: &SingleCard, target: usize) -> CardPlayStats {
  struct Consider<'a> {
    state: &'a CombatState,
    stats: CardPlayStats,
  }
  impl<'a> ConsiderAction for Consider<'a> {
    fn consider(&mut self, action: impl Action) {
      // It theoretically makes more sense to do this on the type level, but that would make the code more complicated, and I'm almost certain this will be optimized out.
      if let DynAction::GainBlockAction(action) = action.clone().into() {
        self.stats.block_amount += action.amount;
      }
      if let DynAction::DamageAction(action) = action.clone().into() {
        if let CreatureIndex::Monster(index) = action.target {
          self.stats.damage[index] += action.info.output as f64;
        }
      }
      if let DynAction::DamageAllEnemiesAction(action) = action.clone().into() {
        for (index, damage) in self.stats.damage.iter_mut().enumerate() {
          *damage += action
            .info
            .apply_target_powers(self.state, CreatureIndex::Monster(index))
            .output as f64;
        }
      }
      if let DynAction::AttackDamageRandomEnemyAction(action) = action.clone().into() {
        let num_monsters = self.state.monsters.iter().filter(|m| !m.gone).count() as f64;
        for (index, damage) in self.stats.damage.iter_mut().enumerate() {
          *damage += action
            .info
            .apply_target_powers(self.state, CreatureIndex::Monster(index))
            .output as f64
            / num_monsters;
        }
      }
    }
  }
  let mut consider = Consider {
    state,
    stats: CardPlayStats {
      block_amount: 0,
      damage: state.monsters.iter().map(|_| 0.0).collect(),
    },
  };
  consider_card_actions(state, card, target, &mut consider);
  consider.stats
}

#[derive(Clone, Debug)]
pub struct CombatResult {
  pub score: f64,
  pub hitpoints_left: i32,
}

impl CombatResult {
  pub fn new(state: &CombatState) -> CombatResult {
    let mut result;
    if state.player.creature.hitpoints > 0 {
      let hitpoint_value = 0.01;
      result = CombatResult {
        score: 1.0 + state.player.creature.hitpoints as f64 * hitpoint_value,
        hitpoints_left: state.player.creature.hitpoints,
      };

      // penalize the player a fixed amount for stolen gold (TODO: choose the value in a more principled way)
      for monster in &state.monsters {
        if monster.gone
          && monster.creature.hitpoints > 0
          && monster.creature.has_power(PowerId::Thievery)
        {
          result.score -= 8.0 * hitpoint_value;
        }
      }
      // ...and extra if you missed out and the combat reward (gold/potion chance) as well
      if state.monsters.iter().all(|m| m.creature.hitpoints > 0) {
        result.score -= 8.0 * hitpoint_value;
      }

      for power in &state.player.creature.powers {
        match power.power_id {
          PowerId::InkBottle => {
            let rewards = [0.0, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 1.0, 2.0, 3.0];
            result.score += rewards[power.amount as usize] * hitpoint_value;
          }
          _ => {}
        }
      }
    } else {
      result = CombatResult {
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

    // slightly penalize the AI for wasting time
    result.score -= state.num_choices as f64 * 0.00000001;
    if state.num_actions >= crate::simulation::HARD_ACTION_LIMIT {
      // greatly penalize the AI for non-winning infinite loops
      result.score -= 2.0;
    }
    result
  }
}
