use enum_map::Enum;
use serde::{Deserialize, Serialize};
use std::convert::From;

use crate::simulation::*;
use crate::simulation_state::monsters::city::ByrdIntent;
use crate::simulation_state::monsters::exordium::TheGuardianIntent;
use crate::simulation_state::monsters::Intent;
use crate::simulation_state::*;
use PowerType::{Buff, Debuff, Relic};

pub struct PowerHookContext<'a, R: Runner> {
  pub runner: &'a mut R,
  pub owner: CreatureIndex,
  pub power_index: usize,
}

pub struct PowerNumericHookContext<'a> {
  pub state: &'a CombatState,
  pub owner: CreatureIndex,
  pub power_index: usize,
}

impl<'a> PowerNumericHookContext<'a> {
  pub fn state(&self) -> &CombatState {
    self.state
  }
  pub fn owner_index(&self) -> CreatureIndex {
    self.owner
  }
  pub fn owner_creature(&self) -> &Creature {
    self.state().get_creature(self.owner)
  }
  pub fn this_power(&self) -> &Power {
    &self.owner_creature().powers[self.power_index]
  }
  pub fn amount(&self) -> i32 {
    self.this_power().amount
  }
}

impl<'a, R: Runner> PowerHookContext<'a, R> {
  pub fn state(&self) -> &CombatState {
    self.runner.state()
  }
  pub fn state_mut(&mut self) -> &mut CombatState {
    self.runner.state_mut()
  }
  pub fn owner_index(&self) -> CreatureIndex {
    self.owner
  }
  pub fn owner_creature(&self) -> &Creature {
    self.state().get_creature(self.owner)
  }
  pub fn owner_creature_mut(&mut self) -> &mut Creature {
    let owner = self.owner;
    self.state_mut().get_creature_mut(owner)
  }
  pub fn this_power(&self) -> &Power {
    &self.owner_creature().powers[self.power_index]
  }
  pub fn amount(&self) -> i32 {
    self.this_power().amount
  }
  pub fn this_power_mut(&mut self) -> &mut Power {
    let power_index = self.power_index;
    &mut self.owner_creature_mut().powers[power_index]
  }
  pub fn remove_just_applied(&mut self) -> bool {
    let power = self.this_power_mut();
    if power.just_applied {
      power.just_applied = false;
      false
    } else {
      true
    }
  }
  pub fn remove_this_power(&mut self) {
    self.action_top(RemoveSpecificPowerAction {
      target: self.owner_index(),
      power_id: self.this_power().power_id,
    });
  }
  pub fn reduce_this_power(&mut self) {
    // why is this a common thing to do in the StS code? ReducePowerAction would already remove it.
    if self.amount() <= 0 {
      self.remove_this_power();
    } else {
      self.action_top(ReducePowerAction {
        target: self.owner_index(),
        power_id: self.this_power().power_id,
        amount: 1,
      });
    }
  }

  pub fn action_top(&mut self, action: impl Action) {
    self.runner.action_top(action);
  }
  pub fn action_bottom(&mut self, action: impl Action) {
    self.runner.action_bottom(action);
  }

  pub fn power_owner_top(&mut self, power_id: PowerId, amount: i32) {
    self.action_top(ApplyPowerAction {
      source: self.owner_index(),
      target: self.owner_index(),
      power_id,
      amount,
    });
  }
  pub fn power_owner_bottom(&mut self, power_id: PowerId, amount: i32) {
    self.action_bottom(ApplyPowerAction {
      source: self.owner_index(),
      target: self.owner_index(),
      power_id,
      amount,
    });
  }
  pub fn power_player_top(&mut self, power_id: PowerId, amount: i32) {
    self.action_top(ApplyPowerAction {
      source: self.owner_index(),
      target: CreatureIndex::Player,
      power_id,
      amount,
    });
  }
  pub fn power_player_bottom(&mut self, power_id: PowerId, amount: i32) {
    self.action_bottom(ApplyPowerAction {
      source: self.owner_index(),
      target: CreatureIndex::Player,
      power_id,
      amount,
    });
  }

  pub fn set_owner_intent(&mut self, intent: impl Intent) {
    if let CreatureIndex::Monster(index) = self.owner_index() {
      self.state_mut().monsters[index]
        .move_history
        .push(intent.id());
    } else {
      panic!("called set_owner_intent on the player")
    }
  }
}

#[allow(unused)]
pub trait PowerBehavior {
  fn inherent_energy(&self) -> i32 {
    0
  }

  fn priority(&self) -> i32 {
    5
  }
  fn stack_power(&self, power: &mut Power, stack_amount: i32) {
    if (power.amount == -1) {
      return;
    }
    power.amount += stack_amount;
  }
  fn reduce_power(&self, power: &mut Power, reduce_amount: i32) {
    power.amount = std::cmp::max(0, power.amount - reduce_amount);
  }

  fn at_damage_give(
    &self,
    context: &PowerNumericHookContext,
    damage: f64,
    damage_type: DamageType,
  ) -> f64 {
    damage
  }
  fn at_damage_final_receive(
    &self,
    context: &PowerNumericHookContext,
    damage: f64,
    damage_type: DamageType,
  ) -> f64 {
    damage
  }
  fn at_damage_receive(
    &self,
    context: &PowerNumericHookContext,
    damage: f64,
    damage_type: DamageType,
  ) -> f64 {
    damage
  }
  fn at_start_of_turn(&self, context: &mut PowerHookContext<impl Runner>) {}
  fn during_turn(&self, context: &mut PowerHookContext<impl Runner>) {}
  fn at_start_of_turn_post_draw(&self, context: &mut PowerHookContext<impl Runner>) {}
  fn at_end_of_turn(&self, context: &mut PowerHookContext<impl Runner>) {}
  fn at_end_of_round(&self, context: &mut PowerHookContext<impl Runner>) {}
  fn on_attacked(
    &self,
    context: &mut PowerHookContext<impl Runner>,
    info: DamageInfoAllPowers,
    damage: i32,
  ) {
  }
  fn on_attack(
    &self,
    context: &mut PowerHookContext<impl Runner>,
    damage: i32,
    target: CreatureIndex,
  ) {
  }
  fn on_attacked_to_change_damage(&self, context: &PowerNumericHookContext, damage: i32) -> i32 {
    damage
  }
  fn on_inflict_damage(&self, context: &mut PowerHookContext<impl Runner>) {}
  fn on_card_draw(&self, context: &mut PowerHookContext<impl Runner>, card: &SingleCard) {}
  fn on_use_card(
    &self,
    context: &mut PowerHookContext<impl Runner>,
    card: &SingleCard,
    action: &mut UseCardAction,
  ) {
  }
  fn on_after_use_card(&self, context: &mut PowerHookContext<impl Runner>, card: &SingleCard) {}
  fn on_specific_trigger(&self, context: &mut PowerHookContext<impl Runner>) {}
  fn on_death(&self, context: &mut PowerHookContext<impl Runner>) {}
  fn at_energy_gain(&self, context: &mut PowerHookContext<impl Runner>) {}
  fn on_exhaust(&self, context: &mut PowerHookContext<impl Runner>, card: &SingleCard) {}
  fn modify_block(&self, context: &PowerNumericHookContext, block: f64) -> f64 {
    block
  }
  fn on_gained_block(&self, context: &mut PowerHookContext<impl Runner>, block: f64) {}
  fn on_remove(&self, context: &mut PowerHookContext<impl Runner>) {}
  fn on_energy_recharge(&self, context: &mut PowerHookContext<impl Runner>) {}
  fn on_after_card_played(&self, context: &mut PowerHookContext<impl Runner>, card: &SingleCard) {}
  fn on_heal(&self, context: &PowerNumericHookContext, amount: i32) -> i32 {
    amount
  }
}

//pub fn

macro_rules! powers {
  ($([$id: expr, $Variant: ident, $power_type: expr],)*) => {
    #[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Enum, Debug)]
    pub enum PowerId {
      $($Variant,)*
    }

    $(
    #[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Debug)]
    pub struct $Variant;)*

    impl From<& str> for PowerId {
      fn from (source: & str)->PowerId {
        match source {
          $($id => PowerId::$Variant,)*
          _ => PowerId::Unknown,
        }
      }
    }

    impl PowerId {
      pub fn power_type(&self)->PowerType {
        match self {
          $(PowerId::$Variant => $power_type,)*
        }
      }
    }

    impl PowerBehavior for PowerId {
      fn inherent_energy(&self) -> i32 {
        match self {
          $(PowerId::$Variant => $Variant.inherent_energy(),)*
        }
      }

      fn priority(&self) -> i32 {
        match self {
          $(PowerId::$Variant => $Variant.priority(),)*
        }
      }
      fn stack_power(&self, power: &mut Power, stack_amount: i32) {
        match self {
          $(PowerId::$Variant => $Variant.stack_power(power, stack_amount),)*
        }
      }
      fn reduce_power(&self, power: &mut Power, reduce_amount: i32) {
        match self {
          $(PowerId::$Variant => $Variant.reduce_power(power, reduce_amount),)*
        }
      }

      fn at_damage_give(
        &self,
        context: &PowerNumericHookContext,
        damage: f64,
        damage_type: DamageType,
      ) -> f64 {
        match self {
          $(PowerId::$Variant => $Variant.at_damage_give(context, damage, damage_type),)*
        }
      }
      fn at_damage_final_receive(
        &self,
        context: &PowerNumericHookContext,
        damage: f64,
        damage_type: DamageType,
      ) -> f64 {
        match self {
          $(PowerId::$Variant => $Variant.at_damage_final_receive(context, damage, damage_type),)*
        }
      }
      fn at_damage_receive(
        &self,
        context: &PowerNumericHookContext,
        damage: f64,
        damage_type: DamageType,
      ) -> f64 {
        match self {
          $(PowerId::$Variant => $Variant.at_damage_receive(context, damage, damage_type),)*
        }
      }
      fn at_start_of_turn(&self, context: &mut PowerHookContext<impl Runner>) {
        match self {
          $(PowerId::$Variant => $Variant.at_start_of_turn(context),)*
        }
      }
      fn during_turn(&self, context: &mut PowerHookContext<impl Runner>) {
        match self {
          $(PowerId::$Variant => $Variant.during_turn(context),)*
        }
      }
      fn at_start_of_turn_post_draw(&self, context: &mut PowerHookContext<impl Runner>) {
        match self {
          $(PowerId::$Variant => $Variant.at_start_of_turn_post_draw(context),)*
        }
      }
      fn at_end_of_turn(&self, context: &mut PowerHookContext<impl Runner>) {
        match self {
          $(PowerId::$Variant => $Variant.at_end_of_turn(context),)*
        }
      }
      fn at_end_of_round(&self, context: &mut PowerHookContext<impl Runner>) {
        match self {
          $(PowerId::$Variant => $Variant.at_end_of_round(context),)*
        }
      }
      fn on_attacked(
        &self,
        context: &mut PowerHookContext<impl Runner>,
        info: DamageInfoAllPowers,
        damage: i32,
      ) {
        match self {
          $(PowerId::$Variant => $Variant.on_attacked(context, info, damage),)*
        }
      }
      fn on_attack(
        &self,
        context: &mut PowerHookContext<impl Runner>,
        damage: i32,
        target: CreatureIndex,
      ) {
        match self {
          $(PowerId::$Variant => $Variant.on_attack(context, damage, target),)*
        }
      }
      fn on_attacked_to_change_damage(&self, context: &PowerNumericHookContext, damage: i32) -> i32 {
        match self {
          $(PowerId::$Variant => $Variant.on_attacked_to_change_damage(context, damage),)*
        }
      }
      fn on_inflict_damage(&self, context: &mut PowerHookContext<impl Runner>) {
        match self {
          $(PowerId::$Variant => $Variant.on_inflict_damage(context),)*
        }
      }
      fn on_card_draw(&self, context: &mut PowerHookContext<impl Runner>, card: &SingleCard) {
        match self {
          $(PowerId::$Variant => $Variant.on_card_draw(context, card),)*
        }
      }
      fn on_use_card(&self, context: &mut PowerHookContext<impl Runner>, card: &SingleCard, action: &mut UseCardAction) {
        match self {
          $(PowerId::$Variant => $Variant.on_use_card(context, card, action),)*
        }
      }
      fn on_after_use_card(&self, context: &mut PowerHookContext<impl Runner>, card: &SingleCard) {
        match self {
          $(PowerId::$Variant => $Variant.on_after_use_card(context, card),)*
        }
      }
      fn on_specific_trigger(&self, context: &mut PowerHookContext<impl Runner>) {
        match self {
          $(PowerId::$Variant => $Variant.on_specific_trigger(context),)*
        }
      }
      fn on_death(&self, context: &mut PowerHookContext<impl Runner>) {
        match self {
          $(PowerId::$Variant => $Variant.on_death(context),)*
        }
      }
      fn at_energy_gain(&self, context: &mut PowerHookContext<impl Runner>) {
        match self {
          $(PowerId::$Variant => $Variant.at_energy_gain(context),)*
        }
      }
      fn on_exhaust(&self, context: &mut PowerHookContext<impl Runner>, card: &SingleCard) {
        match self {
          $(PowerId::$Variant => $Variant.on_exhaust(context, card),)*
        }
      }
      fn modify_block(&self, context: &PowerNumericHookContext, block: f64) -> f64 {
        match self {
          $(PowerId::$Variant => $Variant.modify_block(context, block),)*
        }
      }
      fn on_gained_block(&self, context: &mut PowerHookContext<impl Runner>, block: f64) {
        match self {
          $(PowerId::$Variant => $Variant.on_gained_block(context, block),)*
        }
      }
      fn on_remove(&self, context: &mut PowerHookContext<impl Runner>) {
        match self {
          $(PowerId::$Variant => $Variant.on_remove(context),)*
        }
      }
      fn on_energy_recharge(&self, context: &mut PowerHookContext<impl Runner>) {
        match self {
          $(PowerId::$Variant => $Variant.on_energy_recharge(context),)*
        }
      }
      fn on_after_card_played(&self, context: &mut PowerHookContext<impl Runner>, card: &SingleCard) {
        match self {
          $(PowerId::$Variant => $Variant.on_after_card_played(context, card),)*
        }
      }
    }
  }
}

powers! {
  // Common powers
  ["Dexterity", Dexterity, Buff],
  ["Frail", Frail, Debuff],
  ["Strength", Strength, Buff],
  ["Vulnerable", Vulnerable, Debuff],
  ["Weakened", Weak, Debuff],

  // Less common powers that are still shared with more than one card/relic/monster
  ["Thorns", Thorns, Buff],
  ["Metallicize", Metallicize, Buff],
  ["No Draw", NoDraw, Debuff],
  ["Plated Armor", PlatedArmor, Buff],

  // Common relics
  ["InkBottle", InkBottle, Relic],

  // Boss relics
  ["Busted Crown", BustedCrown, Relic],
  ["Coffee Dripper", CoffeeDripper, Relic],
  ["Cursed Key", CursedKey, Relic],
  ["Ectoplasm", Ectoplasm, Relic],
  ["Fusion Hammer", FusionHammer, Relic],
  ["Mark of Pain", MarkOfPain, Relic],
  ["Philosopher's Stone", PhilosophersStone, Relic],
  ["Runic Pyramid", RunicPyramid, Relic],
  ["SacredBark", SacredBark, Relic],
  ["Sozu", Sozu, Relic],

  // Event relics
  ["Necronomicon", Necronomicon, Relic],

  // Relic powers
  ["Pen Nib", PenNib, Buff],

  // Ironclad uncommon card powers
  ["Combust", Combust, Buff],
  ["Corruption", Corruption, Buff],
  ["Evolve", Evolve, Buff],
  ["Feel No Pain", FeelNoPain, Buff],
  ["Fire Breathing", FireBreathing, Buff],
  ["Flame Barrier", FlameBarrier, Buff],
  ["Rage", Rage, Buff],
  ["Rupture", Rupture, Buff],

  // Ironclad rare card powers
  ["Barricade", Barricade, Buff],
  ["Berserk", Berserk, Buff],
  ["Brutality", Brutality, Buff],
  ["Dark Embrace", DarkEmbrace, Buff],
  ["Demon Form", DemonForm, Buff],
  ["Double Tap", DoubleTap, Buff],
  ["Juggernaut", Juggernaut, Buff],

  // Exordium monster powers
  ["Ritual", Ritual, Buff],
  ["Curl Up", CurlUp, Buff],
  ["Thievery", Thievery, Buff],
  ["Spore Cloud", SporeCloud, Buff],
  ["Entangled", Entangled, Debuff],
  ["Angry", Angry, Buff],
  ["Split", Split, Buff],

  // Exordium elite powers
  ["Anger", Enrage, Buff],
  ["Artifact", Artifact, Buff],

  // Exordium boss powers
  ["Sharp Hide", SharpHide, Buff],
  ["Mode Shift", ModeShift, Buff],
  ["Mode Shift Damage Threshold (not a real power)", ModeShiftDamageThreshold, Buff],

  // City monster powers
  ["Flight", Flight, Buff],


  ["Unknown", Unknown, Buff],
}

impl PowerBehavior for Vulnerable {
  fn at_end_of_round(&self, context: &mut PowerHookContext<impl Runner>) {
    context.reduce_this_power();
  }
  fn at_damage_receive(
    &self,
    _context: &PowerNumericHookContext,
    damage: f64,
    damage_type: DamageType,
  ) -> f64 {
    if damage_type != DamageType::Normal {
      return damage;
    }
    damage * 1.5
  }
}

impl PowerBehavior for Frail {
  fn priority(&self) -> i32 {
    10
  }
  fn at_end_of_round(&self, context: &mut PowerHookContext<impl Runner>) {
    context.reduce_this_power();
  }
  fn modify_block(&self, _context: &PowerNumericHookContext, block: f64) -> f64 {
    block * 0.75
  }
}

impl PowerBehavior for Weak {
  fn priority(&self) -> i32 {
    99
  }
  fn at_end_of_round(&self, context: &mut PowerHookContext<impl Runner>) {
    context.reduce_this_power();
  }
  fn at_damage_give(
    &self,
    _context: &PowerNumericHookContext,
    damage: f64,
    damage_type: DamageType,
  ) -> f64 {
    if damage_type != DamageType::Normal {
      return damage;
    }
    damage * 0.75
  }
}

impl PowerBehavior for Strength {
  fn stack_power(&self, power: &mut Power, stack_amount: i32) {
    power.amount += stack_amount;
    if power.amount > 999 {
      power.amount = 999;
    }
    if power.amount < -999 {
      power.amount = -999;
    }
  }
  fn reduce_power(&self, power: &mut Power, reduce_amount: i32) {
    self.stack_power(power, -reduce_amount);
  }
  fn at_damage_give(
    &self,
    context: &PowerNumericHookContext,
    damage: f64,
    damage_type: DamageType,
  ) -> f64 {
    if damage_type != DamageType::Normal {
      return damage;
    }
    damage + context.amount() as f64
  }
}

impl PowerBehavior for Dexterity {
  fn stack_power(&self, power: &mut Power, stack_amount: i32) {
    power.amount += stack_amount;
    if power.amount > 999 {
      power.amount = 999;
    }
    if power.amount < -999 {
      power.amount = -999;
    }
  }
  fn reduce_power(&self, power: &mut Power, reduce_amount: i32) {
    self.stack_power(power, -reduce_amount);
  }
  fn modify_block(&self, context: &PowerNumericHookContext, block: f64) -> f64 {
    block + context.amount() as f64
  }
}

impl PowerBehavior for Thorns {
  fn on_attacked(
    &self,
    context: &mut PowerHookContext<impl Runner>,
    info: DamageInfoAllPowers,
    _damage: i32,
  ) {
    if let Some(attacker) = info.owner {
      if attacker != context.owner_index() && info.damage_type == DamageType::Normal {
        context.action_top(DamageAction {
          target: attacker,
          info: DamageInfoNoPowers::new(
            Some(context.owner_index()),
            context.amount(),
            DamageType::Thorns,
          )
          .ignore_powers(),
        });
      }
    }
  }
}

impl PowerBehavior for Ritual {
  fn at_end_of_round(&self, context: &mut PowerHookContext<impl Runner>) {
    if context.remove_just_applied() {
      context.power_owner_bottom(PowerId::Strength, context.amount());
    }
  }
}

impl PowerBehavior for CurlUp {
  fn on_attacked(
    &self,
    context: &mut PowerHookContext<impl Runner>,
    info: DamageInfoAllPowers,
    damage: i32,
  ) {
    // hack: using amount == 0 instead of this.triggered
    if context.amount() > 0
      && damage < context.owner_creature().hitpoints
      && info.damage_type == DamageType::Normal
    {
      context.action_bottom(GainBlockAction {
        creature_index: context.owner_index(),
        amount: context.amount(),
      });
      context.this_power_mut().amount = 0;
      context.remove_this_power();
    }
  }
}

impl PowerBehavior for Thievery {}

impl PowerBehavior for SporeCloud {
  fn on_death(&self, context: &mut PowerHookContext<impl Runner>) {
    context.power_player_top(PowerId::Vulnerable, context.amount());
  }
}

impl PowerBehavior for Entangled {
  fn at_end_of_round(&self, context: &mut PowerHookContext<impl Runner>) {
    context.remove_this_power();
  }
}

impl PowerBehavior for Angry {
  fn on_attacked(
    &self,
    context: &mut PowerHookContext<impl Runner>,
    info: DamageInfoAllPowers,
    damage: i32,
  ) {
    if info.owner == Some(CreatureIndex::Player)
      && damage > 0
      && info.damage_type == DamageType::Normal
    {
      context.power_owner_top(PowerId::Strength, context.amount());
    }
  }
}

impl PowerBehavior for Enrage {
  fn on_use_card(
    &self,
    context: &mut PowerHookContext<impl Runner>,
    card: &SingleCard,
    _action: &mut UseCardAction,
  ) {
    if card.card_info.card_type == CardType::Skill {
      context.power_owner_top(PowerId::Strength, context.amount());
    }
  }
}

impl PowerBehavior for Artifact {
  fn on_specific_trigger(&self, context: &mut PowerHookContext<impl Runner>) {
    context.reduce_this_power();
  }
}

impl PowerBehavior for Metallicize {
  fn at_end_of_turn(&self, context: &mut PowerHookContext<impl Runner>) {
    context.action_bottom(GainBlockAction {
      creature_index: context.owner_index(),
      amount: context.amount(),
    });
  }
}

impl PowerBehavior for SharpHide {
  fn on_use_card(
    &self,
    context: &mut PowerHookContext<impl Runner>,
    card: &SingleCard,
    _action: &mut UseCardAction,
  ) {
    if card.card_info.card_type == CardType::Attack {
      context.action_bottom(DamageAction {
        target: CreatureIndex::Player,
        info: DamageInfoNoPowers::new(
          Some(context.owner_index()),
          context.amount(),
          DamageType::Thorns,
        )
        .ignore_powers(),
      });
    }
  }
}

impl PowerBehavior for ModeShift {
  // Note: In StS, this behavior is actually part of TheGuardian.damage and not ModeShift
  fn on_attacked(
    &self,
    context: &mut PowerHookContext<impl Runner>,
    _info: DamageInfoAllPowers,
    damage: i32,
  ) {
    if damage > 0 {
      context.action_top(ReducePowerAction {
        target: context.owner_index(),
        power_id: context.this_power().power_id,
        amount: damage,
      });
    }
  }

  fn on_remove(&self, context: &mut PowerHookContext<impl Runner>) {
    context.action_bottom(GainBlockAction {
      creature_index: context.owner_index(),
      amount: 20,
    });
    context.set_owner_intent(TheGuardianIntent::DefensiveMode);
    context.power_owner_top(PowerId::ModeShiftDamageThreshold, 10);
  }
}
impl PowerBehavior for ModeShiftDamageThreshold {}

impl PowerBehavior for Flight {
  fn at_damage_final_receive(
    &self,
    _context: &PowerNumericHookContext,
    damage: f64,
    damage_type: DamageType,
  ) -> f64 {
    if damage_type != DamageType::Normal {
      return damage;
    }
    damage / 2.0
  }

  fn on_attacked(
    &self,
    context: &mut PowerHookContext<impl Runner>,
    info: DamageInfoAllPowers,
    damage: i32,
  ) {
    if info.owner.is_some() && damage > 0 && info.damage_type == DamageType::Normal {
      context.reduce_this_power();
    }
  }

  fn at_start_of_turn(&self, context: &mut PowerHookContext<impl Runner>) {
    let deficiency = context.this_power().misc - context.amount();
    if deficiency > 0 {
      context.power_owner_top(PowerId::Flight, deficiency);
    }
  }

  fn on_remove(&self, context: &mut PowerHookContext<impl Runner>) {
    context.set_owner_intent(ByrdIntent::Stunned);
  }
}

impl PowerBehavior for NoDraw {
  fn at_end_of_turn(&self, context: &mut PowerHookContext<impl Runner>) {
    context.remove_this_power();
  }
}

impl PowerBehavior for PlatedArmor {
  fn at_end_of_turn(&self, context: &mut PowerHookContext<impl Runner>) {
    context.action_bottom(GainBlockAction {
      creature_index: context.owner_index(),
      amount: context.this_power().amount,
    });
  }

  fn on_attacked(
    &self,
    context: &mut PowerHookContext<impl Runner>,
    info: DamageInfoAllPowers,
    damage: i32,
  ) {
    if damage > 0 && info.damage_type == DamageType::Normal {
      context.reduce_this_power();
    }
  }
}

impl PowerBehavior for InkBottle {
  fn on_use_card(
    &self,
    context: &mut PowerHookContext<impl Runner>,
    _card: &SingleCard,
    _action: &mut UseCardAction,
  ) {
    let amount = &mut context.this_power_mut().amount;
    *amount += 1;
    if *amount == 10 {
      *amount = 0;
      context.action_bottom(DrawCards(1));
    }
  }
}

macro_rules! energy_relic {
  () => {
    fn inherent_energy(&self) -> i32 {
      1
    }
  };
}

impl PowerBehavior for BustedCrown {
  energy_relic! {}
}
impl PowerBehavior for CoffeeDripper {
  energy_relic! {}
}
impl PowerBehavior for CursedKey {
  energy_relic! {}
}
impl PowerBehavior for Ectoplasm {
  energy_relic! {}
}
impl PowerBehavior for FusionHammer {
  energy_relic! {}
}
impl PowerBehavior for MarkOfPain {
  energy_relic! {}
}
impl PowerBehavior for PhilosophersStone {
  energy_relic! {}
}
impl PowerBehavior for RunicPyramid {}
impl PowerBehavior for SacredBark {}
impl PowerBehavior for Sozu {
  energy_relic! {}
}

impl PowerBehavior for Necronomicon {
  fn at_start_of_turn(&self, context: &mut PowerHookContext<impl Runner>) {
    context.this_power_mut().amount = -1;
  }
  fn on_use_card(
    &self,
    context: &mut PowerHookContext<impl Runner>,
    card: &SingleCard,
    action: &mut UseCardAction,
  ) {
    if context.amount() != 0
      && card.card_info.card_type == CardType::Attack
      && card.cost_in_practice(context.state()) >= 2
    {
      context.this_power_mut().amount = 0;
      let mut new_action = UseCardAction::new(card.clone(), action.target, context.state());
      new_action.purge_on_use = true;
      new_action.energy_on_use = action.energy_on_use;
      context.state_mut().card_queue.push_back(new_action);
    }
  }
}

impl PowerBehavior for PenNib {
  fn priority(&self) -> i32 {
    6
  }
  fn on_use_card(
    &self,
    context: &mut PowerHookContext<impl Runner>,
    card: &SingleCard,
    _action: &mut UseCardAction,
  ) {
    if card.card_info.card_type == CardType::Attack {
      context.remove_this_power();
    }
  }
  fn at_damage_give(
    &self,
    _context: &PowerNumericHookContext,
    damage: f64,
    damage_type: DamageType,
  ) -> f64 {
    if damage_type != DamageType::Normal {
      return damage;
    }
    damage * 2.0
  }
}

impl PowerBehavior for Combust {
  //TODO
}

impl PowerBehavior for Corruption {
  //TODO
}

impl PowerBehavior for Evolve {
  fn on_card_draw(&self, context: &mut PowerHookContext<impl Runner>, card: &SingleCard) {
    if card.card_info.card_type == CardType::Status {
      context.action_bottom(DrawCards(context.amount()));
    }
  }
}

impl PowerBehavior for FeelNoPain {
  fn on_exhaust(&self, context: &mut PowerHookContext<impl Runner>, _card: &SingleCard) {
    context.action_bottom(GainBlockAction {
      creature_index: context.owner_index(),
      amount: context.amount(),
    });
  }
}

impl PowerBehavior for FireBreathing {
  //TODO
}

impl PowerBehavior for FlameBarrier {
  fn on_attacked(
    &self,
    context: &mut PowerHookContext<impl Runner>,
    info: DamageInfoAllPowers,
    damage: i32,
  ) {
    Thorns.on_attacked(context, info, damage)
  }
  fn at_start_of_turn(&self, context: &mut PowerHookContext<impl Runner>) {
    context.remove_this_power();
  }
}

impl PowerBehavior for Rage {
  fn on_use_card(
    &self,
    context: &mut PowerHookContext<impl Runner>,
    card: &SingleCard,
    _action: &mut UseCardAction,
  ) {
    if card.card_info.card_type == CardType::Attack {
      context.action_bottom(GainBlockAction {
        creature_index: context.owner_index(),
        amount: context.amount(),
      });
    }
  }
  fn at_end_of_turn(&self, context: &mut PowerHookContext<impl Runner>) {
    context.remove_this_power();
  }
}

impl PowerBehavior for Rupture {
  //TODO
}

impl PowerBehavior for Barricade {}

impl PowerBehavior for Berserk {
  fn at_start_of_turn(&self, context: &mut PowerHookContext<impl Runner>) {
    context.action_bottom(GainEnergyAction(context.amount()))
  }
}

impl PowerBehavior for Brutality {
  //TODO
}

impl PowerBehavior for DarkEmbrace {
  fn on_exhaust(&self, context: &mut PowerHookContext<impl Runner>, _card: &SingleCard) {
    context.action_bottom(DrawCards(1));
  }
}

impl PowerBehavior for DemonForm {
  fn at_start_of_turn_post_draw(&self, context: &mut PowerHookContext<impl Runner>) {
    context.power_owner_bottom(PowerId::Strength, context.amount());
  }
}

impl PowerBehavior for DoubleTap {
  //TODO
}

impl PowerBehavior for Juggernaut {
  fn on_gained_block(&self, _context: &mut PowerHookContext<impl Runner>, _block: f64) {
    //TODO
  }
}

impl PowerBehavior for Split {}
impl PowerBehavior for Unknown {}
