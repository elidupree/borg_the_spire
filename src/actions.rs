#![allow (unused_variables)]

use retain_mut::RetainMut;
use serde::{Deserialize, Serialize};
use std::convert::From;

use crate::simulation::*;
use crate::simulation_state::cards::PlayCardContext;
use crate::simulation_state::monsters::DoIntentContext;
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
      fn execute(& self, runner: &mut Runner) {
        match self {
          $(DynAction::$Variant (value) => value.execute(runner),)*
        }
      }
      fn execute_random (& self, runner: &mut Runner, random_value: i32) {
        match self {
          $(DynAction::$Variant (value) => value.execute_random (runner, random_value),)*
        }
      }
    }
  }
}


//Note: not every `&mut state` or `&mut Runner` function needs to be a Action, but such a function needs to be a Action if it EITHER uses direct randomness OR needs to be possible to queue up due to coming immediately after something that might be nondeterministic.

actions! {
  // mainly used by the engine
  // TODO: make these match the actual game a bit closer
  [PlayCard {pub card: SingleCard, pub target: usize}],
  [FinishPlayingCard;],
  [EndTurn;],
  [StartMonsterTurn (pub usize);],
  [DoMonsterIntent (pub usize);],
  [FinishMonsterTurn (pub usize);],
  [FinishCreatureTurn (pub CreatureIndex);],
  [ChooseMonsterIntent (pub usize);],

  // used by many effects
  [DrawCardRandom;],
  [DrawCards (pub i32);],
  [TakeHit {pub creature_index: CreatureIndex, base_damage: i32}],
  [ApplyPowerAction {pub source: CreatureIndex, pub target: CreatureIndex, pub power_id: PowerId, pub amount: i32}],
  [ReducePowerAction {pub target: CreatureIndex, pub power_id: PowerId, pub amount: i32}],
  [RemoveSpecificPowerAction {pub target: CreatureIndex, pub power_id: PowerId}],
  [DiscardNewCard (pub SingleCard);],

  // generally card effects
  [AttackMonster {pub base_damage: i32, pub swings: i32, pub target: usize}],
  [AttackMonsters {pub base_damage: i32, pub swings: i32}],
  [GainBlockAction {pub creature_index: CreatureIndex, pub amount: i32}],
  
  // generally monster effects
  [InitializeMonsterInnateDamageAmount{pub monster_index: usize, pub range: (i32, i32)}],
  [AttackPlayer {pub monster_index: usize, pub base_damage: i32, pub swings: i32}],
}

impl Action for PlayCard {
  fn execute(&self, runner: &mut Runner) {
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

    runner.apply(&FinishPlayingCard);
  }
}

impl Action for FinishPlayingCard {
  fn execute (&self, runner: &mut Runner) {
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
  fn execute(&self, runner: &mut Runner) {
    let state = runner.state_mut();
    for card in state.hand.drain(..) {
      if card.card_info.ethereal {
        state.exhaust_pile.push(card);
      } else {
        state.discard_pile.push(card);
      }
    }
    runner.apply(&FinishCreatureTurn(CreatureIndex::Player));

    runner.apply(&StartMonsterTurn(0));
  }
}

impl Action for StartMonsterTurn {
  fn execute (&self, runner: &mut Runner) {
    if let Some(monster) = runner.state_mut().monsters.get_mut(self.0) {
      if !monster.gone {
        monster.creature.start_turn();
      }
      if !runner.state().combat_over() {
        runner.apply(&StartMonsterTurn(self.0 + 1));
      }
    } else {
      runner.apply(&DoMonsterIntent(0));
    }
  }
}

impl Action for DoMonsterIntent {
  fn execute(&self, runner: &mut Runner) {
    if let Some(monster) = runner.state().monsters.get(self.0) {
      let monster_id = monster.monster_id;
      if !monster.gone {
        monster_id.intent_effects(&mut DoIntentContext::new(runner, self.0));
      }
      if !runner.state().combat_over() {
        runner.apply(&DoMonsterIntent(self.0 + 1));
      }
    } else {
      runner.apply(&FinishMonsterTurn(0));
    }
  }
}

impl Action for FinishMonsterTurn {
  fn execute(&self, runner: &mut Runner) {
    if let Some(monster) = runner.state_mut().monsters.get_mut(self.0) {
      if !monster.gone {
        runner.apply(&FinishCreatureTurn(CreatureIndex::Monster(self.0)));
        runner.apply(&ChooseMonsterIntent(self.0));
      }
      if !runner.state().combat_over() {
        runner.apply(&FinishMonsterTurn(self.0 + 1));
      }
    } else {
      let state = runner.state_mut();
      state.player.creature.finish_round();
      state.player.creature.start_turn();
      state.player.energy = 3;
      runner.apply(&DrawCards(5));
    }
  }
}

impl Action for FinishCreatureTurn {
  fn execute(&self, runner: &mut Runner) {
    let creature = runner.state_mut().get_creature_mut(self.0);
    match self.0 {
      CreatureIndex::Monster(_) => creature.finish_round(),
      _ => (),
    }
    for index in 0..creature.powers.len() {
      match runner.state().get_creature(self.0).powers[index].power_id {
        PowerId::Ritual => {
          //TODO: this is buggy, doing mutable actions after applying
          if runner.state().get_creature(self.0).powers[index].just_applied {
            runner.state_mut().get_creature_mut(self.0).powers[index].just_applied = false;
          } else {
            runner.apply(&ApplyPowerAction {
              source: self.0, target: self.0,
              power_id: PowerId::Strength,
              amount: runner.state().get_creature(self.0).powers[index].amount,
            });
          }
        }
        _ => (),
      }
    }
  }
}

impl Action for ChooseMonsterIntent {
  fn determinism(&self, state: &CombatState) -> Determinism {
    Determinism::Random(monsters::intent_choice_distribution(state, self.0))
  }
  fn execute_random(&self, runner: &mut Runner, random_value: i32) {
    let monster = &mut runner.state_mut().monsters[self.0];
    if !monster.gone {
      let monster_id = monster.monster_id;
      monster.push_intent(random_value);
      monster_id.after_choosing_intent(runner, self.0);
    }
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
  fn execute_random(&self, runner: &mut Runner, random_value: i32) {
    let card = runner.state_mut().draw_pile.remove(random_value as usize);
    runner.state_mut().hand.push(card);
  }
}

impl Action for DrawCards {
  fn execute(&self, runner: &mut Runner) {
    //TODO: more nuanced
    if self.0 <= 0 {
      return;
    }
    let state = runner.state_mut();
    if state.hand.len() == 10 {
      return;
    }
    if state.draw_pile.is_empty() {
      std::mem::swap(&mut state.draw_pile, &mut state.discard_pile);
    }
    if !state.draw_pile.is_empty() {
      runner.apply(&DrawCardRandom);
      runner.apply(&DrawCards(self.0 - 1));
    }
  }
}

impl Action for TakeHit {
  fn execute(&self, runner: &mut Runner) {
    let mut creature = runner.state_mut().get_creature_mut(self.creature_index);
    let mut damage = creature.adjusted_damage_received(self.base_damage);

    if creature.block >= damage {
      creature.block -= damage;
    } else {
      damage -= creature.block;
      creature.block = 0;
      creature.hitpoints -= damage;
      let block = &mut creature.block;
      creature.powers.retain_mut(|power| match power.power_id {
        PowerId::CurlUp => {
          *block += power.amount;
          false
        }
        _ => true,
      });
      if creature.hitpoints <= 0 {
        creature.hitpoints = 0;
        if let CreatureIndex::Monster(monster_index) = self.creature_index {
          runner.state_mut().monsters[monster_index].gone = true;
        }
      }
    }
  }
}

impl Action for ApplyPowerAction {
  fn execute(&self, runner: &mut Runner) {
    if let CreatureIndex::Monster (monster_index) = self.target {
      if runner.state().monsters [monster_index].gone {
        return;
      }
    }
    
    // TODO: Snecko Skull, Champion Belt, Ginger, Turnip
    
    if runner.state().get_creature (self.target).has_power (PowerId::Artifact) &&
      self.power_id.power_type() == PowerType::Debuff {
      power_hook!(runner, self.target, PowerId::Artifact, on_specific_trigger());
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
      target.powers.sort_by_key (|power| power.power_id.priority());
    }
  }
}



impl Action for ReducePowerAction {
  fn execute(&self, runner: &mut Runner) {
    let target = runner.state_mut().get_creature_mut(self.target);
    let existing = target
      .powers
      .iter_mut()
      .find(|power| power.power_id == self.power_id);
    if let Some(existing) = existing {
      if self.amount <existing.amount {
        existing.power_id.clone().reduce_power(existing, self.amount);
      } else {
        runner.action_top (RemoveSpecificPowerAction {target: self.target, power_id: self.power_id});
      }
    }
  }
}


impl Action for RemoveSpecificPowerAction {
  fn execute(&self, runner: &mut Runner) {
    let target = runner.state_mut().get_creature_mut(self.target);
    target.powers.retain (| power | power.power_id != self.power_id);
  }
}

impl Action for GainBlockAction {
  fn execute(&self, runner: &mut Runner) {
    let creature = runner.state_mut().get_creature_mut(self.creature_index);
    creature.do_block(self.amount);
  }
}

impl Action for DiscardNewCard {
  fn execute(&self, runner: &mut Runner) {
    runner.state_mut().discard_pile.push(self.0.clone());
  }
}

impl Action for AttackMonster {
  fn execute(&self, runner: &mut Runner) {
    let monster = &runner.state().monsters[self.target];
    if monster.gone {
      return;
    }
    let adjusted_damage = runner
      .state()
      .player
      .creature
      .adjusted_damage_dealt(self.base_damage);
    runner.apply(&TakeHit {
      creature_index: CreatureIndex::Monster(self.target),
      base_damage: adjusted_damage,
    });
    if self.swings > 1 {
      runner.apply(&AttackMonster {
        base_damage: self.base_damage,
        swings: self.swings - 1,
        target: self.target,
      });
    }
  }
}

impl Action for AttackMonsters {
  fn execute(&self, runner: &mut Runner) {
    for monster_index in 0..runner.state().monsters.len() {
      let monster = &runner.state().monsters[monster_index];
      if !monster.gone {
        let adjusted_damage = runner
          .state()
          .player
          .creature
          .adjusted_damage_dealt(self.base_damage);
        runner.apply(&TakeHit {
          creature_index: CreatureIndex::Monster(monster_index),
          base_damage: adjusted_damage,
        });
      }
    }
    if self.swings > 1 {
      runner.apply(&AttackMonsters {
        base_damage: self.base_damage,
        swings: self.swings - 1,
      });
    }
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
  fn execute_random(&self, runner: &mut Runner, random_value: i32) {
    let mut monster = &mut runner.state_mut().monsters[self.monster_index];
    monster.innate_damage_amount = Some(random_value);
  }
}
impl Action for AttackPlayer {
  fn execute(&self, runner: &mut Runner) {
    let monster = &runner.state().monsters[self.monster_index];
    if monster.gone {
      return;
    }
    let adjusted_damage = monster.creature.adjusted_damage_dealt(self.base_damage);
    runner.apply(&TakeHit {
      creature_index: CreatureIndex::Player,
      base_damage: adjusted_damage,
    });
    if self.swings > 1 {
      runner.apply(&AttackPlayer {
        base_damage: self.base_damage,
        swings: self.swings - 1,
        monster_index: self.monster_index,
      });
    }
  }
}
