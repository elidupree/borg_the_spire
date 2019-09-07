use rand::{Rng, SeedableRng};
use rand_xoshiro::Xoshiro256StarStar;
use retain_mut::RetainMut;
use std::any::Any;
type Generator = Xoshiro256StarStar;

pub use crate::simulation_state::cards::CardBehavior;
pub use crate::simulation_state::monsters::MonsterBehavior;
use crate::simulation_state::*;

pub trait Runner {
  fn gen<F: FnOnce(&mut Generator) -> i32>(&mut self, f: F) -> i32;
}

pub struct DefaultRunner {
  generator: Generator,
  values: Vec<i32>,
}

impl Runner for DefaultRunner {
  fn gen<F: FnOnce(&mut Generator) -> i32>(&mut self, f: F) -> i32 {
    let result = (f)(&mut self.generator);
    self.values.push(result);
    result
  }
}

pub struct ReplayRunner<'a> {
  values: &'a [i32],
  position: usize,
}

impl<'a> Runner for ReplayRunner<'a> {
  fn gen<F: FnOnce(&mut Generator) -> i32>(&mut self, _f: F) -> i32 {
    let current = self.position;
    self.position += 1;
    self
      .values
      .get(current)
      .expect("ReplayRunner was prompted for a more values than originally")
      .clone()
  }
}

impl Creature {
  pub fn apply_power_amount(&mut self, power_id: PowerId, amount: i32) {
    let existing = self
      .powers
      .iter_mut()
      .find(|power| power.power_id == power_id);
    if let Some(existing) = existing {
      existing.amount += amount;
    } else {
      self.powers.push(Power {
        power_id,
        amount,
        ..Default::default()
      });
    }
  }

  pub fn has_power(&self, power_id: PowerId) -> bool {
    self.powers.iter().any(|power| power.power_id == power_id)
  }
  pub fn power_amount(&self, power_id: PowerId) -> i32 {
    self
      .powers
      .iter()
      .filter(|power| power.power_id == power_id)
      .map(|power| power.amount)
      .sum()
  }

  pub fn start_turn(&mut self) {
    self.block = 0;
    self.powers.retain_mut(|power| match power.power_id {
      PowerId::Vulnerable | PowerId::Weak | PowerId::Frail => {
        power.amount -= 1;
        power.amount > 0
      }
      _ => true,
    });
  }

  pub fn finish_turn(&mut self) {
    self.block = 0;
    for index in 0..self.powers.len() {
      match self.powers[index].power_id {
        PowerId::Ritual => {
          if self.powers[index].just_applied {
            self.powers[index].just_applied = false;
          } else {
            self.apply_power_amount(PowerId::Strength, self.powers[index].amount);
          }
        }
        _ => (),
      }
    }
  }

  pub fn adjusted_damage_received(&self, mut damage: i32) -> i32 {
    if self.has_power(PowerId::Vulnerable) {
      damage = (damage * 3 + 1) / 2;
    }
    damage
  }

  pub fn adjusted_damage_dealt(&self, mut damage: i32) -> i32 {
    damage += self.power_amount(PowerId::Strength);
    if self.has_power(PowerId::Weak) {
      damage = (damage * 3) / 4;
    }
    if damage <= 0 {return 0;}
    damage
  }

  pub fn take_hit(&mut self, mut damage: i32) {
    damage = self.adjusted_damage_received(damage);
    
    if self.block >= damage {
      self.block -= damage;
    } else {
      damage -= self.block;
      self.block = 0;
      self.hitpoints -= damage;
      let block = &mut self.block;
      self.powers.retain_mut(|power| match power.power_id {
        PowerId::CurlUp => {
          *block += power.amount;
          false
        }
        _ => true,
      });
    }
  }
  
  pub fn do_block (&mut self, amount: i32) {
    if amount >0 {self.block += amount;}
  }
}

impl CombatState {
  pub fn draw_card(&mut self, runner: &mut impl Runner) {
    if self.hand.len() == 10 {
      return;
    }
    if self.draw_pile.is_empty() {
      std::mem::swap(&mut self.draw_pile, &mut self.discard_pile);
    }
    if !self.draw_pile.is_empty() {
      let index =
        runner.gen(|generator| generator.gen_range(0, self.draw_pile.len() as i32)) as usize;
      let card = self.draw_pile.remove(index);
      self.hand.push(card);
    }
  }

  pub fn start_player_turn(&mut self, runner: &mut impl Runner) {
    self.player.creature.start_turn();
    self.player.energy = 3;
    for _ in 0..5 {
      self.draw_card(runner);
    }
  }

  pub fn finish_player_turn(&mut self, runner: &mut impl Runner) {
    for card in self.hand.drain(..) {
      if card.card_info.ethereal {
        self.exhaust_pile.push(card);
      } else {
        self.discard_pile.push(card);
      }
    }
    self.player.creature.finish_turn();
  }

  pub fn end_turn(&mut self, runner: &mut impl Runner) {
    self.finish_player_turn(runner);
    for monster in self.monsters.iter_mut() {
      if!monster.gone {monster.creature.start_turn();}
    }

    for index in 0..self.monsters.len() {
      if!self.monsters [index].gone {
       self.enact_monster_intent(runner, index);
      if self.player.creature.hitpoints <= 0 {
        return;
      }}
    }

    for monster in self.monsters.iter_mut() {
      if!monster.gone { monster.creature.finish_turn();
      monster.choose_next_intent(runner);
      }
    }
  }

  pub fn play_card(&mut self, runner: &mut impl Runner, card_index: usize, target: usize) {
    let card = self.hand.remove(card_index);
    let card_id = card.card_info.id;
    self.player.energy -= card.cost;
    self.card_in_play = Some(card);

    card_id.play(self, runner, target);

    let card = self.card_in_play.take().unwrap();
    if card.card_info.card_type == CardType::Power {
      // card disappears
    } else if card.card_info.exhausts {
      self.exhaust_pile.push(card);
    } else {
      self.discard_pile.push(card);
    }
  }
  pub fn enact_monster_intent(&mut self, runner: &mut impl Runner, monster_index: usize) {
    let monster_id = self.monsters[monster_index].monster_id;

    monster_id.enact_intent(self, runner, monster_index);
  }

  pub fn monster_intent(&self, monster_index: usize) -> i32 {
    *self.monsters[monster_index].move_history.last().unwrap()
  }

  pub fn monster_attacks_player(
    &mut self,
    runner: &mut impl Runner,
    monster_index: usize,
    damage: i32,
    swings: i32,
  ) {
    let monster = &mut self.monsters[monster_index];
    for _ in 0..swings {
      self
        .player
        .creature
        .take_hit(monster.creature.adjusted_damage_dealt(damage));
      if self.player.creature.hitpoints <= 0 {
        break;
      }
    }
  }
  
  pub fn player_attacks_monster (
    &mut self,
    runner: &mut impl Runner,
    monster_index: usize,
    damage: i32,
    swings: i32,
  ) {
    let monster = &mut self.monsters[monster_index];
    for _ in 0..swings {
      monster
        .creature
        .take_hit(self.player.creature.adjusted_damage_dealt(damage));
      if monster.creature.hitpoints <= 0 {
        monster.gone = true;
        break;
      }
    }
  }
  
  pub fn player_attacks_all_monsters (
    &mut self,
    runner: &mut impl Runner,
    damage: i32,
    swings: i32,
  ) {
    
    for _ in 0..swings {
      for monster in &mut self.monsters {
      monster
        .creature
        .take_hit(self.player.creature.adjusted_damage_dealt(damage));
      if monster.creature.hitpoints <= 0 {
        monster.gone = true;
        break;
      }}
    }
  }
}

impl Monster {
  pub fn choose_next_intent(&mut self, runner: &mut impl Runner) {
    let monster_id = self.monster_id;

    monster_id.choose_next_intent(self, runner);
  }

  pub fn intent(&self) -> i32 {
    *self.move_history.last().unwrap()
  }
  pub fn last_move(&self, intent: i32) -> bool {
    self.move_history.last() == Some(&intent)
  }
  pub fn last_two_moves(&self, intent: i32) -> bool {
    self.move_history.len() >= 2
      && self.move_history[self.move_history.len() - 2..] == [intent, intent]
  }
}
