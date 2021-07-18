#![allow(unused_variables)]

use arrayvec::ArrayVec;
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;
use std::convert::From;

use crate::seed_system::Distribution;
use crate::simulation::*;
use crate::simulation_state::cards::PlayCardContext;
use crate::simulation_state::monsters::DoIntentContext;
use crate::simulation_state::powers::PowerBehavior;
use crate::simulation_state::*;

macro_rules! actions {
  ($([$Variant: ident $($struct_body: tt)*],)*) => {
    #[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Debug)]
    pub enum DynAction {
      $($Variant ($Variant),)*
    }

    $(#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Debug)]
    pub struct $Variant $($struct_body)*

    impl From<$Variant> for DynAction {
      fn from (source: $Variant)->DynAction {
        DynAction::$Variant (source)
      }
    }
    )*

    impl Action for DynAction {
      fn determinism (& self, state: &CombatState)->Determinism {
        match self {
          $(DynAction::$Variant (value) => value.determinism (state),)*
        }
      }
      fn execute(& self, runner: &mut impl Runner) {
        match self {
          $(DynAction::$Variant (value) => value.execute(runner),)*
        }
      }
      fn execute_random (& self, runner: &mut impl Runner, random_value: i32) {
        match self {
          $(DynAction::$Variant (value) => value.execute_random (runner, random_value),)*
        }
      }
    }
  }
}

//Note: not every `&mut state` or `&mut impl Runner` function needs to be a Action, but such a function needs to be a Action if it EITHER uses direct randomness OR needs to be possible to queue up due to coming immediately after something that might be nondeterministic.

actions! {
  // mainly used by the engine
  // TODO: make these match the actual game a bit closer
  [PlayCard {pub card: SingleCard, pub target: usize}],
  [FinishPlayingCard;],
  [EndTurn;],
  [StartMonsterTurn (pub usize);],
  [DoMonsterIntent (pub usize);],
  [FinishMonsterTurn (pub usize);],
  [ChooseMonsterIntent (pub usize);],
  [EndMonstersTurns;],

  // used by many effects
  [DamageAction {pub target: CreatureIndex, pub info: DamageInfoAllPowers}],
  [DamageAllEnemiesAction {pub info: DamageInfoOwnerPowers}],
  [AttackDamageRandomEnemyAction {pub info: DamageInfoOwnerPowers}],
  [DrawCardRandom;],
  [DrawCards (pub i32);],
  [ApplyPowerAction {pub source: CreatureIndex, pub target: CreatureIndex, pub power_id: PowerId, pub amount: i32}],
  [ReducePowerAction {pub target: CreatureIndex, pub power_id: PowerId, pub amount: i32}],
  [RemoveSpecificPowerAction {pub target: CreatureIndex, pub power_id: PowerId}],
  [DiscardNewCard (pub SingleCard);],
  [GainBlockAction {pub creature_index: CreatureIndex, pub amount: i32}],
  [GainEnergyAction (pub i32);],

  // generally card effects
  [ArmamentsAction {pub upgraded: bool}],

  // generally monster effects
  [InitializeMonsterInnateDamageAmount{pub monster_index: usize, pub range: (i32, i32)}],
  [GainBlockRandomMonsterAction {pub source: usize, pub amount: i32}],
  [SplitAction (pub usize, pub [MonsterId; 2]);],
  [EscapeAction (pub usize);],
}

impl Action for PlayCard {
  fn execute(&self, runner: &mut impl Runner) {
    power_hook!(runner, AllCreatures, on_use_card(&self.card.clone()));
    let state = runner.state_mut();
    let card_index = state.hand.iter().position(|c| *c == self.card).unwrap();
    let card = state.hand.remove(card_index);
    let card_id = card.card_info.id;
    state.player.energy -= card.cost;
    state.card_in_play = Some(card);

    card_id.behavior(&mut PlayCardContext {
      runner,
      target: self.target,
    });

    runner.action_now(&FinishPlayingCard);
  }
}

impl Action for FinishPlayingCard {
  fn execute(&self, runner: &mut impl Runner) {
    let state = runner.state_mut();
    let card = state.card_in_play.take().unwrap();
    if card.card_info.card_type == CardType::Power {
      // card disappears
    } else if card.card_info.exhausts {
      state.exhaust_pile.push(card);
    } else {
      state.discard_pile.push(card);
    }
  }
}

impl Action for EndTurn {
  fn execute(&self, runner: &mut impl Runner) {
    power_hook!(runner, CreatureIndex::Player, at_end_of_turn());

    let state = runner.state_mut();
    state.turn_has_ended = true;
    let mut actions: ArrayVec<DamageAction, 10> = ArrayVec::new();
    for card in state.hand.drain(..) {
      if card.card_info.id == CardId::Burn {
        actions.push(DamageAction {
          target: CreatureIndex::Player,
          info: DamageInfoNoPowers::new(
            CreatureIndex::Player,
            2 + card.upgrades * 2,
            DamageType::Thorns,
          )
          .ignore_powers(),
        });
      }
      if card.card_info.ethereal {
        state.exhaust_pile.push(card);
      } else {
        state.discard_pile.push(card);
      }
    }
    for action in actions {
      runner.action_bottom(action);
    }

    runner.action_now(&StartMonsterTurn(0));
  }
}

pub fn apply_end_of_turn_powers(runner: &mut impl Runner) {
  power_hook!(runner, AllMonsters, at_end_of_turn());
  power_hook!(runner, AllCreatures, at_end_of_round());
}
pub fn start_creature_turn(runner: &mut impl Runner, creature_index: CreatureIndex) {
  power_hook!(runner, creature_index, at_start_of_turn());
  let creature = runner.state_mut().get_creature_mut(creature_index);
  if !creature.has_power(PowerId::Barricade) {
    creature.block = 0;
  }
  // TODO: make this actually post-draw
  power_hook!(runner, creature_index, at_start_of_turn_post_draw());
}

impl Action for StartMonsterTurn {
  fn execute(&self, runner: &mut impl Runner) {
    if let Some(monster) = runner.state_mut().monsters.get_mut(self.0) {
      if !monster.gone {
        start_creature_turn(runner, CreatureIndex::Monster(self.0));
      }
      if !runner.state().combat_over() {
        runner.action_now(&StartMonsterTurn(self.0 + 1));
      }
    } else {
      runner.action_now(&DoMonsterIntent(0));
    }
  }
}

impl Action for DoMonsterIntent {
  fn execute(&self, runner: &mut impl Runner) {
    if let Some(monster) = runner.state().monsters.get(self.0) {
      let monster_id = monster.monster_id;
      if !monster.gone {
        monster_id.intent_effects(&mut DoIntentContext::new(runner, self.0));
      }
      if !runner.state().combat_over() {
        runner.action_now(&DoMonsterIntent(self.0 + 1));
      }
    } else {
      runner.action_bottom(FinishMonsterTurn(0));
    }
  }
}

impl Action for FinishMonsterTurn {
  fn execute(&self, runner: &mut impl Runner) {
    if let Some(monster) = runner.state_mut().monsters.get_mut(self.0) {
      if !monster.gone {
        runner.action_now(&ChooseMonsterIntent(self.0));
      }
      if !runner.state().combat_over() {
        runner.action_bottom(FinishMonsterTurn(self.0 + 1));
      }
    } else {
      runner.action_now(&EndMonstersTurns);
    }
  }
}

impl Action for EndMonstersTurns {
  fn execute(&self, runner: &mut impl Runner) {
    apply_end_of_turn_powers(runner);
    let state = runner.state_mut();
    state.turn_number += 1;
    state.turn_has_ended = false;
    start_creature_turn(runner, CreatureIndex::Player);
    let state = runner.state_mut();
    state.player.energy = 3
      + state
        .player
        .creature
        .powers
        .iter()
        .map(|power| power.power_id.inherent_energy())
        .sum::<i32>();
    runner.action_now(&DrawCards(5));
  }
}

impl Action for ChooseMonsterIntent {
  fn determinism(&self, state: &CombatState) -> Determinism {
    match monsters::intent_choice_distribution(state, self.0) {
      Some(distribution) => Determinism::Random(distribution),
      None => Determinism::Deterministic,
    }
  }
  fn execute(&self, _runner: &mut impl Runner) {
    // intent_choice_distribution returned None, meaning "don't change intent"
  }
  fn execute_random(&self, runner: &mut impl Runner, random_value: i32) {
    let monster = &mut runner.state_mut().monsters[self.0];
    if !monster.gone {
      let monster_id = monster.monster_id;
      monster.push_intent(random_value as IntentId);
      monster_id.after_choosing_intent(runner, self.0);
    }
  }
}

impl Action for DamageAction {
  fn execute(&self, runner: &mut impl Runner) {
    let mut damage = self.info.output;
    //TODO: intangible
    if damage < 0 {
      damage = 0;
    }

    let target = runner.state_mut().get_creature_mut(self.target);
    if damage >= target.block {
      damage -= target.block;
      target.block = 0;
    } else {
      target.block -= damage;
      damage = 0;
    }

    // TODO: various relic hooks
    power_hook!(
      runner.state(),
      self.target,
      damage = on_attacked_to_change_damage(damage)
    );
    power_hook!(runner, self.target, on_attacked(self.info.clone(), damage));

    let target = runner.state_mut().get_creature_mut(self.target);
    target.hitpoints -= damage;
    if target.hitpoints <= 0 {
      target.hitpoints = 0;
      match self.target {
        CreatureIndex::Player => {}
        CreatureIndex::Monster(monster_index) => {
          runner.state_mut().monsters[monster_index].gone = true;
          power_hook!(runner, self.target, on_death());
        }
      }
    }
  }
}

impl Action for DamageAllEnemiesAction {
  fn execute(&self, runner: &mut impl Runner) {
    for monster_index in 0..runner.state().monsters.len() {
      if !runner.state().monsters[monster_index].gone {
        let target = CreatureIndex::Monster(monster_index);
        let info = self.info.apply_target_powers(runner.state(), target);
        runner.action_now(&DamageAction { target, info });
      }
    }
  }
}

impl Action for AttackDamageRandomEnemyAction {
  fn determinism(&self, state: &CombatState) -> Determinism {
    Determinism::Random(Distribution(
      state
        .monsters
        .iter()
        .enumerate()
        .filter(|(index, monster)| !monster.gone)
        .map(|(index, monster)| (1.0, index as i32))
        .collect(),
    ))
  }
  fn execute_random(&self, runner: &mut impl Runner, random_value: i32) {
    // hack: this is not quite where powers are applied to card/monster damage in the actual code
    let target = CreatureIndex::Monster(random_value as usize);
    let info = self.info.apply_target_powers(runner.state(), target);
    runner.action_now(&DamageAction { target, info });
  }
}

impl Action for DrawCardRandom {
  fn determinism(&self, state: &CombatState) -> Determinism {
    Determinism::Random(Distribution(
      (0..state.draw_pile.len() as i32)
        .map(|index| (1.0, index))
        .collect(),
    ))
  }
  fn execute_random(&self, runner: &mut impl Runner, random_value: i32) {
    let card = runner.state_mut().draw_pile.remove(random_value as usize);
    runner.state_mut().hand.push(card);
  }
}

impl Action for DrawCards {
  fn execute(&self, runner: &mut impl Runner) {
    if runner.state().player.creature.has_power(PowerId::NoDraw) {
      return;
    }

    //TODO: more nuanced
    if self.0 <= 0 {
      return;
    }
    let state = runner.state_mut();
    if state.hand.len() == 10 {
      return;
    }
    if state.draw_pile.is_empty() {
      state.num_reshuffles += 1;
      // I considered sorting to prevent some rare kinds of draw order manipulation,
      // but it turned out to cause about a 4% slowdown in playouts, which seems troublesome
      //state.discard_pile.sort();
      std::mem::swap(&mut state.draw_pile, &mut state.discard_pile);
    }
    if !state.draw_pile.is_empty() {
      runner.action_now(&DrawCardRandom);
      runner.action_now(&DrawCards(self.0 - 1));
    }
  }
}

impl Action for ApplyPowerAction {
  fn execute(&self, runner: &mut impl Runner) {
    if let CreatureIndex::Monster(monster_index) = self.target {
      if runner.state().monsters[monster_index].gone {
        return;
      }
    }

    // TODO: Snecko Skull, Champion Belt, Ginger, Turnip

    if runner
      .state()
      .get_creature(self.target)
      .has_power(PowerId::Artifact)
      && self.power_id.power_type() == PowerType::Debuff
    {
      power_hook!(
        runner,
        self.target,
        PowerId::Artifact,
        on_specific_trigger()
      );
      return;
    }

    let just_applied = runner.state().turn_has_ended;

    //if this.source == CreatureIndex::Player && this.target != this.source && {
    let target = runner.state_mut().get_creature_mut(self.target);
    let existing = target
      .powers
      .iter_mut()
      .find(|power| power.power_id == self.power_id);
    // TODO: not for Nightmare

    if let Some(existing) = existing {
      existing.power_id.clone().stack_power(existing, self.amount);
    } else {
      target.powers.push(Power {
        power_id: self.power_id,
        amount: self.amount,
        just_applied,
        ..Default::default()
      });
      target.powers.sort_by_key(|power| power.power_id.priority());
    }
  }
}

impl Action for ReducePowerAction {
  fn execute(&self, runner: &mut impl Runner) {
    let target = runner.state_mut().get_creature_mut(self.target);
    let existing = target
      .powers
      .iter_mut()
      .find(|power| power.power_id == self.power_id);
    if let Some(existing) = existing {
      if self.amount < existing.amount {
        existing
          .power_id
          .clone()
          .reduce_power(existing, self.amount);
      } else {
        runner.action_top(RemoveSpecificPowerAction {
          target: self.target,
          power_id: self.power_id,
        });
      }
    }
  }
}

impl Action for RemoveSpecificPowerAction {
  fn execute(&self, runner: &mut impl Runner) {
    let target = runner.state_mut().get_creature_mut(self.target);
    target
      .powers
      .retain(|power| power.power_id != self.power_id);
  }
}

impl Action for GainBlockAction {
  fn execute(&self, runner: &mut impl Runner) {
    let creature = runner.state_mut().get_creature_mut(self.creature_index);
    if self.amount > 0 {
      creature.block += self.amount;
    }
  }
}

impl Action for GainEnergyAction {
  fn execute(&self, runner: &mut impl Runner) {
    runner.state_mut().player.energy += self.0;
  }
}

impl Action for DiscardNewCard {
  fn execute(&self, runner: &mut impl Runner) {
    runner.state_mut().discard_pile.push(self.0.clone());
  }
}

impl Action for ArmamentsAction {
  fn determinism(&self, state: &CombatState) -> Determinism {
    if self.upgraded {
      Determinism::Deterministic
    } else {
      // TODO: Determinism::Choice
      Determinism::Random(Distribution(
        (0..state.hand.len()).map(|i| (1.0, i as i32)).collect(),
      ))
    }
  }

  fn execute(&self, runner: &mut impl Runner) {
    if self.upgraded {
      for card in &mut runner.state_mut().hand {
        card.upgrade()
      }
    }
  }
  fn execute_random(&self, runner: &mut impl Runner, random_value: i32) {
    runner.state_mut().hand[random_value as usize].upgrade()
  }
}

impl Action for InitializeMonsterInnateDamageAmount {
  fn determinism(&self, state: &CombatState) -> Determinism {
    Determinism::Random(Distribution(
      (self.range.0..self.range.1)
        .map(|index| (1.0, index))
        .collect(),
    ))
  }
  fn execute_random(&self, runner: &mut impl Runner, random_value: i32) {
    let mut monster = &mut runner.state_mut().monsters[self.monster_index];
    monster.innate_damage_amount = Some(random_value);
  }
}

impl Action for GainBlockRandomMonsterAction {
  fn determinism(&self, state: &CombatState) -> Determinism {
    let others: SmallVec<_> = state
      .monsters
      .iter()
      .enumerate()
      .filter(|&(index, monster)| index != self.source && !monster.gone)
      .map(|(index, monster)| (1.0, index as i32))
      .collect();
    Determinism::Random(if others.is_empty() {
      Distribution::from(self.source as i32)
    } else {
      Distribution(others)
    })
  }
  fn execute_random(&self, runner: &mut impl Runner, random_value: i32) {
    let creature = &mut runner.state_mut().monsters[random_value as usize].creature;
    if self.amount > 0 {
      creature.block += self.amount;
    }
  }
}

impl Action for SplitAction {
  fn execute(&self, runner: &mut impl Runner) {
    let &SplitAction(index, ids) = self;
    let state = runner.state_mut();
    let splitting = &mut state.monsters[index];

    let new_monsters: [Monster; 2] = ids.map(|monster_id| Monster {
      monster_id,
      innate_damage_amount: None,
      ascension: splitting.ascension,
      move_history: Vec::new(),
      gone: false,
      creature: Creature {
        hitpoints: splitting.creature.hitpoints,
        max_hitpoints: splitting.creature.hitpoints,
        block: 0,
        powers: Vec::new(),
      },
    });

    splitting.creature.hitpoints = 0;
    splitting.gone = true;
    runner
      .state_mut()
      .monsters
      .extend(new_monsters.iter().cloned());
  }
}

impl Action for EscapeAction {
  fn execute(&self, runner: &mut impl Runner) {
    let state = runner.state_mut();
    let escaping = &mut state.monsters[self.0];

    escaping.gone = true;
  }
}
