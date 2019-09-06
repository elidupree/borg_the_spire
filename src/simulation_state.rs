use std::sync::Arc;
use std::convert::From;
use serde::{Deserialize, Serialize};
use rand::{Rng, SeedableRng};
use rand_xoshiro::Xoshiro256StarStar;
type Generator = Xoshiro256StarStar;

use crate::communication_mod_state as communication;

pub mod cards;
pub mod powers;
pub mod monsters;

pub use cards::CardId;
pub use monsters::MonsterId;
pub use powers::PowerId;

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct CombatState {
  pub draw_pile: Vec<SingleCard>,
  pub discard_pile: Vec<SingleCard>,
  pub exhaust_pile: Vec<SingleCard>,
  pub hand: Vec<SingleCard>,
  pub limbo: Vec<SingleCard>,
  pub player: Player,
  pub monsters: Vec<Monster>,
}

#[derive(Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Debug)]
pub struct SingleCard {
  pub misc: i32,
  pub cost: i32,
  pub upgrades: i32,
  pub card_info: Arc<CardInfo>,
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Debug)]
pub enum CardType {
  Attack, Skill, Power, Status, Curse
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Debug)]
pub enum Rarity {
  Common, Uncommon, Rare, Basic, Special
}

#[derive(Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Debug)]
pub struct CardInfo {
  pub id: CardId,
  pub card_type: CardType,
  pub rarity: Rarity,
  pub normal_cost: i32,
  pub upgraded_cost: i32,
  pub ethereal: bool,
  pub has_target: bool,
  pub exhausts: bool,
}

impl Default for CardInfo {
  fn default ()->CardInfo {
    CardInfo {
      id: CardId::Injury,
      card_type: CardType::Curse,
      rarity: Rarity::Special,
      normal_cost: -2,
      upgraded_cost: -3,
      ethereal: false,
      has_target: false,
      exhausts: false,
    }
  }
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Creature {
  pub hitpoints: i32,
  pub max_hitpoints: i32,
  pub block: i32,
  pub powers: Vec<Power>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Player {
  pub creature: Creature,
  pub energy: i32,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Monster {
  pub original_index: usize,
  pub monster_id: MonsterId,
  pub innate_damage_amount: Option <i32>,
  pub ascension: i32,
  pub creature: Creature,
  pub move_history: Vec<i32>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Power {
  pub power_id: PowerId,
  pub amount: i32,
  #[serde(default)]
  pub damage: i32,
  pub card: Option <SingleCard>,
  #[serde(default)]
  pub misc: i32,
  pub just_applied: bool,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Relic {
  name: String,
  id: String,
  counter: i32,
}


impl CombatState {
  pub fn from_communication_mod (observed: & communication::GameState, previous: Option <& CombatState>)->Option <CombatState> {
    let combat = observed.combat_state.as_ref()?;
    
    let mut result = CombatState {
      draw_pile: combat.draw_pile.iter().map (From::from).collect(),
      discard_pile: combat.discard_pile.iter().map (From::from).collect(),
      exhaust_pile: combat.exhaust_pile.iter().map (From::from).collect(),
      hand: combat.hand.iter().map (From::from).collect(),
      limbo: combat.limbo.iter().map (From::from).collect(),
      player: Player::from (& combat.player),
      monsters: combat.monsters.iter().enumerate().map (| (index, monster) | {
        let mut move_history = vec![monster.move_id];
        if let Some(previous) = monster.last_move_id {
          move_history.insert (0,previous);
        }
        if let Some(previous) = monster.second_last_move_id {
          move_history.insert (0,previous);
        }
        let innate_damage_amount = if monster.move_base_damage > 0 {Some(monster.move_base_damage)} else {None};
        Monster {
          original_index: index,
          monster_id: MonsterId::from (&*monster.id),
          ascension: observed.ascension_level,
          move_history,
          innate_damage_amount,
          creature: Creature {
            hitpoints: monster.current_hp,
            max_hitpoints: monster.max_hp,
            block: monster.block,
            powers: monster.powers.iter().map (From::from).collect(),
          },
        }
      }).collect(),
    };
    
    if let Some(previous) = previous {
      for monster in &previous.monsters {
        for new_version in result.monsters.iter_mut() {
          if new_version.original_index == monster.original_index {
            new_version.innate_damage_amount = monster.innate_damage_amount;
          }
        }
      }
    }
    Some (result)
  }
}

impl From <& communication::Card> for SingleCard {
  fn from (card: & communication::Card)->SingleCard {
    SingleCard {
      misc: card.misc,
      cost: card.cost,
      upgrades: card.upgrades,
      card_info: Arc::new (CardInfo::from (CardId::from (&*card.id))),
    }
  }
}

impl From <& communication::Power> for Power {
  fn from (power: & communication::Power)->Power {
    Power {
      power_id: PowerId::from (&*power.id),
      amount: power.amount,
      damage: power.damage,
      card: power.card.as_ref().map (From::from),
      misc: power.misc,
      just_applied: power.just_applied,
    }
  }
}

impl From <& communication::Player> for Player {
  fn from (player: & communication::Player)->Player {
    Player{
        energy: player.energy,
        creature: Creature {
            hitpoints: player.current_hp,
            max_hitpoints: player.max_hp,
            block: player.block,
            powers: player.powers.iter().map (From::from).collect(),
          },
      }
  }
}
