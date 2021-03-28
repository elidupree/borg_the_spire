use enum_map::Enum;
use serde::{Deserialize, Serialize};
use std::convert::From;

use crate::simulation::*;
use crate::simulation_state::*;
use PowerType::{Buff, Debuff, Relic};

pub struct PowerHookContext<'a, 'b> {
  pub runner: &'a mut Runner<'b>,
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

impl<'a, 'b> PowerHookContext<'a, 'b> {
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
  fn at_start_of_turn(&self, context: &mut PowerHookContext) {}
  fn during_turn(&self, context: &mut PowerHookContext) {}
  fn at_start_of_turn_post_draw(&self, context: &mut PowerHookContext) {}
  fn at_end_of_turn(&self, context: &mut PowerHookContext) {}
  fn at_end_of_round(&self, context: &mut PowerHookContext) {}
  fn on_attacked(&self, context: &mut PowerHookContext, info: DamageInfo, damage: i32) {}
  fn on_attack(&self, context: &mut PowerHookContext, damage: i32, target: CreatureIndex) {}
  fn on_attacked_to_change_damage(&self, context: &PowerNumericHookContext, damage: i32) -> i32 {
    damage
  }
  fn on_inflict_damage(&self, context: &mut PowerHookContext) {}
  fn on_card_draw(&self, context: &mut PowerHookContext, card: &SingleCard) {}
  fn on_use_card(&self, context: &mut PowerHookContext, card: &SingleCard) {}
  fn on_after_use_card(&self, context: &mut PowerHookContext, card: &SingleCard) {}
  fn on_specific_trigger(&self, context: &mut PowerHookContext) {}
  fn on_death(&self, context: &mut PowerHookContext) {}
  fn at_energy_gain(&self, context: &mut PowerHookContext) {}
  fn on_exhaust(&self, context: &mut PowerHookContext, card: &SingleCard) {}
  fn modify_block(&self, context: &PowerNumericHookContext, block: f64) -> f64 {
    block
  }
  fn on_gained_block(&self, context: &mut PowerHookContext, block: f64) {}
  fn on_remove(&self, context: &mut PowerHookContext) {}
  fn on_energy_recharge(&self, context: &mut PowerHookContext) {}
  fn on_after_card_played(&self, context: &mut PowerHookContext, card: &SingleCard) {}
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

    impl PowerId {
      pub fn power_type(&self)->PowerType {
        match self {
          $(PowerId::$Variant => $power_type,)*
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

  // Relics
  ["Busted Crown", BustedCrown, Relic],
  ["Coffee Dripper", CoffeeDripper, Relic],
  ["Cursed Key", CursedKey, Relic],
  ["Ectoplasm", Ectoplasm, Relic],
  ["Fusion Hammer", FusionHammer, Relic],
  ["Mark of Pain", MarkOfPain, Relic],
  ["Philosopher's Stone", PhilosophersStone, Relic],
  ["Sozu", Sozu, Relic],

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

  // City monster powers
  ["Flight", Flight, Buff],


  ["Unknown", Unknown, Buff],
}

impl PowerBehavior for Vulnerable {
  fn at_end_of_round(&self, context: &mut PowerHookContext) {
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
  fn at_end_of_round(&self, context: &mut PowerHookContext) {
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
  fn at_end_of_round(&self, context: &mut PowerHookContext) {
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
  fn on_attacked(&self, context: &mut PowerHookContext, info: DamageInfo, damage: i32) {
    if info.owner != context.owner_index() && info.damage_type == DamageType::Normal {
      context.action_top(DamageAction {
        target: info.owner,
        info: DamageInfo::new(context.owner_index(), context.amount(), DamageType::Thorns),
      });
    }
  }
}

impl PowerBehavior for Ritual {
  fn at_end_of_round(&self, context: &mut PowerHookContext) {
    if context.remove_just_applied() {
      context.power_owner_bottom(PowerId::Strength, context.amount());
    }
  }
}

impl PowerBehavior for CurlUp {
  fn on_attacked(&self, context: &mut PowerHookContext, info: DamageInfo, damage: i32) {
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
  fn on_death(&self, context: &mut PowerHookContext) {
    context.power_player_top(PowerId::Vulnerable, context.amount());
  }
}

impl PowerBehavior for Entangled {
  fn at_end_of_round(&self, context: &mut PowerHookContext) {
    context.remove_this_power();
  }
}

impl PowerBehavior for Angry {
  fn on_attacked(&self, context: &mut PowerHookContext, info: DamageInfo, damage: i32) {
    if info.owner == CreatureIndex::Player && damage > 0 && info.damage_type == DamageType::Normal {
      context.power_owner_top(PowerId::Strength, context.amount());
    }
  }
}

impl PowerBehavior for Enrage {
  fn on_use_card(&self, context: &mut PowerHookContext, card: &SingleCard) {
    if card.card_info.card_type == CardType::Skill {
      context.power_owner_top(PowerId::Strength, context.amount());
    }
  }
}

impl PowerBehavior for Artifact {
  fn on_specific_trigger(&self, context: &mut PowerHookContext) {
    context.reduce_this_power();
  }
}

impl PowerBehavior for Metallicize {
  fn at_end_of_turn(&self, context: &mut PowerHookContext) {
    context.action_bottom(GainBlockAction {
      creature_index: context.owner_index(),
      amount: context.amount(),
    });
  }
}

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

  fn on_attacked(&self, context: &mut PowerHookContext, info: DamageInfo, damage: i32) {
    if damage > 0 && info.damage_type == DamageType::Normal {
      context.reduce_this_power();
    }
  }

  fn at_start_of_turn(&self, context: &mut PowerHookContext) {
    let deficiency = context.this_power().misc - context.amount();
    if deficiency > 0 {
      context.power_owner_top(PowerId::Flight, deficiency);
    }
  }
}

impl PowerBehavior for NoDraw {
  fn at_end_of_turn(&self, context: &mut PowerHookContext) {
    context.remove_this_power();
  }
}

impl PowerBehavior for PlatedArmor {
  fn at_end_of_turn(&self, context: &mut PowerHookContext) {
    context.action_bottom(GainBlockAction {
      creature_index: context.owner_index(),
      amount: context.this_power().amount,
    });
  }

  fn on_attacked(&self, context: &mut PowerHookContext, info: DamageInfo, damage: i32) {
    if damage > 0 && info.damage_type == DamageType::Normal {
      context.reduce_this_power();
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
impl PowerBehavior for Sozu {
  energy_relic! {}
}

impl PowerBehavior for PenNib {
  fn priority(&self) -> i32 {
    6
  }
  fn on_use_card(&self, context: &mut PowerHookContext, card: &SingleCard) {
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
  fn on_card_draw(&self, context: &mut PowerHookContext, card: &SingleCard) {
    if card.card_info.card_type == CardType::Status {
      context.action_bottom(DrawCards(context.amount()));
    }
  }
}

impl PowerBehavior for FeelNoPain {
  fn on_exhaust(&self, context: &mut PowerHookContext, card: &SingleCard) {
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
  fn on_attacked(&self, context: &mut PowerHookContext, info: DamageInfo, damage: i32) {
    Thorns.on_attacked(context, info, damage)
  }
  fn at_start_of_turn(&self, context: &mut PowerHookContext) {
    context.remove_this_power();
  }
}

impl PowerBehavior for Rage {
  fn on_use_card(&self, context: &mut PowerHookContext, card: &SingleCard) {
    if card.card_info.card_type == CardType::Attack {
      context.action_bottom(GainBlockAction {
        creature_index: context.owner_index(),
        amount: context.amount(),
      });
    }
  }
  fn at_end_of_turn(&self, context: &mut PowerHookContext) {
    context.remove_this_power();
  }
}

impl PowerBehavior for Rupture {
  //TODO
}

impl PowerBehavior for Barricade {}

impl PowerBehavior for Berserk {
  fn at_start_of_turn(&self, context: &mut PowerHookContext) {
    context.action_bottom(GainEnergyAction(context.amount()))
  }
}

impl PowerBehavior for Brutality {
  //TODO
}

impl PowerBehavior for DarkEmbrace {
  fn on_exhaust(&self, context: &mut PowerHookContext, card: &SingleCard) {
    context.action_bottom(DrawCards(1));
  }
}

impl PowerBehavior for DemonForm {
  fn at_start_of_turn_post_draw(&self, context: &mut PowerHookContext) {
    context.power_owner_bottom(PowerId::Strength, context.amount());
  }
}

impl PowerBehavior for DoubleTap {
  //TODO
}

impl PowerBehavior for Juggernaut {
  fn on_gained_block(&self, context: &mut PowerHookContext, block: f64) {
    //TODO
  }
}

impl PowerBehavior for Split {}
impl PowerBehavior for Unknown {}
