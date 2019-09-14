use serde::{Deserialize, Serialize};
use std::convert::From;

use crate::simulation_state::*;

pub struct PowerHookContext<'a> {
  pub runner: Runner<'a>,
  pub owner: CreatureIndex,
  pub power_index: usize,
}

impl<'a> PowerHookContext<'a> {
  pub fn state (&self)->&CombatState {
    self.runner.state()
  }
  pub fn state_mut (&mut self)->&mut CombatState {
    self.runner.state_mut()
  }
  pub fn owner_index (&self)->CreatureIndex {self.owner}
  pub fn owner_creature(&self)->&Creature {
    self.state().get_creature(self.owner)
  }
  pub fn owner_creature_mut (&mut self)->&mut Creature {
    let owner = self.owner;
    self.state_mut().get_creature_mut (owner)
  }
  pub fn this_power(&self)->&Power {
    &self.owner_creature().powers [self.power_index]
  }
  pub fn this_power_mut (&mut self)->&mut Power {
    let power_index = self.power_index;
    &mut self.owner_creature_mut ().powers [power_index]
  }
  pub fn remove_just_applied(&mut self)->bool {
    let power = self.this_power_mut();
    if power.just_applied {
      power.just_applied = false;
      false
    }
    else {
      true
    }
  }
  pub fn remove_this_power (&mut self) {
    unimplemented!()
  }
  pub fn reduce_this_power (&mut self) {
    unimplemented!()
  }
  
  pub fn action_top (&mut self, action: impl Action) {
    self.runner.apply (& action);
  }
  pub fn action_bottom (&mut self, action: impl Action) {
    self.runner.apply (& action);
  }
}

#[allow(unused)]
pub trait PowerBehavior {
  fn at_damage_give (&self, context: &mut PowerHookContext, damage: f64, damage_type: DamageType)->f64 {damage}
  fn at_damage_final_receive (&self, context: &mut PowerHookContext, damage: f64, damage_type: DamageType)->f64 {damage}
  fn at_damage_receive (&self, context: &mut PowerHookContext, damage: f64, damage_type: DamageType)->f64 {damage}
  fn at_start_of_turn (&self, context: &mut PowerHookContext) {}
  fn during_turn (&self, context: &mut PowerHookContext) {}
  fn at_start_of_turn_post_draw (&self, context: &mut PowerHookContext) {}
  fn at_end_of_turn (&self, context: &mut PowerHookContext) {}
  fn at_end_of_round (&self, context: &mut PowerHookContext) {}
  fn on_attacked (&self, context: &mut PowerHookContext, info: DamageInfo, damage: i32)->i32 {damage}
  fn on_attack (&self, context: &mut PowerHookContext, damage: i32, target: CreatureIndex) {}
  fn on_attacked_to_change_damage (&self, context: &mut PowerHookContext, damage: i32)->i32 {damage}
  fn on_inflict_damage (&self, context: &mut PowerHookContext) {}
  fn on_card_draw (&self, context: &mut PowerHookContext, card: &SingleCard) {}
  fn on_use_card (&self, context: &mut PowerHookContext, card: &SingleCard) {}
  fn on_after_use_card (&self, context: &mut PowerHookContext, card: &SingleCard) {}
  fn on_death (&self, context: &mut PowerHookContext) {}
  fn at_energy_gain (&self, context: &mut PowerHookContext) {}
  fn on_exhaust (&self, context: &mut PowerHookContext, card: &SingleCard) {}
  fn modify_block (&self, context: &mut PowerHookContext, block: f64)->f64 {block}
  fn on_gained_block (&self, context: &mut PowerHookContext, block: f64) {}
  fn on_remove (&self, context: &mut PowerHookContext) {}
  fn on_energy_recharge (&self, context: &mut PowerHookContext) {}
  fn on_after_card_played (&self, context: &mut PowerHookContext, card: &SingleCard) {}
}

//pub fn

macro_rules! powers {
  ($([$id: expr, $Variant: ident],)*) => {
    #[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Debug)]
    pub enum PowerId {
      $($Variant,)*
    }
    
    $(
    #[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Debug)]
    pub struct $Variant;)*
        
    impl std::ops::Deref for PowerId {
      type Target = dyn PowerBehavior;
      fn deref (&self)->&'static dyn PowerBehavior {
        match self {
          $(PowerId::$Variant => &$Variant,)*
        }
      }
    }

    impl From<& str> for PowerId {
      fn from (source: & str)->PowerId {
        match source {
          $($id => PowerId::$Variant,)*
          _ => PowerId::Unknown,
        }
      }
    }
  }
}

powers! {
  ["Vulnerable", Vulnerable],
  ["Frail", Frail],
  ["Weakened", Weak],
  ["Strength", Strength],
  ["Dexterity", Dexterity],

  ["Ritual", Ritual],
  ["Curl Up", CurlUp],
  ["Thievery", Thievery],
  ["SporeCloud", SporeCloud],
  ["Entangled", Entangled],
  ["Angry", Angry],
  ["Anger", Enrage],

  ["Unknown", Unknown],
}

impl PowerBehavior for Vulnerable {
  fn at_end_of_round (&self, context: &mut PowerHookContext) {
    context.reduce_this_power();
  }
  fn at_damage_receive (&self, _context: &mut PowerHookContext, damage: f64, damage_type: DamageType)->f64 {
    if damage_type != DamageType::Normal {return damage}
    damage*1.5
  }
}

impl PowerBehavior for Frail{
  fn at_end_of_round (&self, context: &mut PowerHookContext) {
    context.reduce_this_power();
  }
  fn modify_block (&self, _context: &mut PowerHookContext, block: f64)->f64 {
    block*0.75
  }
}

impl PowerBehavior for Weak{
  fn at_end_of_round (&self, context: &mut PowerHookContext) {
    context.reduce_this_power();
  }
  fn at_damage_receive (&self, _context: &mut PowerHookContext, damage: f64, damage_type: DamageType)->f64 {
    if damage_type != DamageType::Normal {return damage}
    damage*1.5
  }
}

impl PowerBehavior for Strength{
  fn at_damage_give(&self, context: &mut PowerHookContext, damage: f64, damage_type: DamageType)->f64 {
    if damage_type != DamageType::Normal {return damage}
    damage + context.this_power().amount as f64
  }
}

impl PowerBehavior for Dexterity{
  fn modify_block (&self, context: &mut PowerHookContext, block: f64)->f64 {
    block + context.this_power().amount as f64
  }
}

impl PowerBehavior for Ritual{
  fn at_end_of_round (&self, context: &mut PowerHookContext) {
    if context.remove_just_applied() {
    context.action_bottom (ApplyPowerAmount {
      power_id: PowerId::Strength,
      creature_index: context.owner_index(),
      amount: context.this_power().amount,
      just_applied: false,
    });
    }
  }
}

impl PowerBehavior for CurlUp{
  fn on_attacked (&self, context: &mut PowerHookContext, info: DamageInfo, damage: i32)->i32 {
    // hack: using amount == 0 instead of this.triggered
    if context.this_power().amount >0 && damage <context.owner_creature().hitpoints && info.damage_type == DamageType::Normal {
      context.action_bottom (GainBlockAction {
        creature_index: context.owner_index(),
        amount: context.this_power().amount,
      });
      context.this_power_mut().amount = 0;
      context.remove_this_power();
    }
    damage
  }
}

impl PowerBehavior for Thievery{}

impl PowerBehavior for SporeCloud{}

impl PowerBehavior for Entangled{
  fn at_end_of_round (&self, context: &mut PowerHookContext) {
    context.remove_this_power();
  }
}

impl PowerBehavior for Angry{}

impl PowerBehavior for Enrage{}

impl PowerBehavior for Unknown{}

