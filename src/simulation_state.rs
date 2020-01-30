use arrayvec::ArrayVec;
use derivative::Derivative;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::convert::From;
use std::fmt::{self, Debug, Formatter};
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use crate::actions::*;
use crate::communication_mod_state as communication;
use crate::simulation::*;

pub mod cards;
pub mod monsters;
pub mod powers;

pub use cards::CardId;
pub use monsters::MonsterId;
pub use powers::PowerId;

pub const MAX_MONSTERS: usize = 7;
pub const X_COST: i32 = -1;
pub const UNPLAYABLE: i32 = -2;

pub fn hash_cards_unordered<H: Hasher>(cards: &[SingleCard], hasher: &mut H) {
  let mut sorted: Vec<_> = cards.iter().collect();
  sorted.sort();
  sorted.hash(hasher);
}

pub fn compare_cards_unordered(first: &[SingleCard], second: &[SingleCard]) -> bool {
  let mut first_sorted: Vec<_> = first.iter().collect();
  first_sorted.sort();
  let mut second_sorted: Vec<_> = second.iter().collect();
  second_sorted.sort();
  first_sorted == second_sorted
}

#[derive(Clone, Serialize, Deserialize, Debug, Derivative)]
#[derivative(PartialEq, Eq, Hash)]
pub struct CombatState {
  #[derivative(
    PartialEq(compare_with = "compare_cards_unordered"),
    Hash(hash_with = "hash_cards_unordered")
  )]
  pub draw_pile: Vec<SingleCard>,
  #[derivative(
    PartialEq(compare_with = "compare_cards_unordered"),
    Hash(hash_with = "hash_cards_unordered")
  )]
  pub discard_pile: Vec<SingleCard>,
  #[derivative(
    PartialEq(compare_with = "compare_cards_unordered"),
    Hash(hash_with = "hash_cards_unordered")
  )]
  pub exhaust_pile: Vec<SingleCard>,
  #[derivative(
    PartialEq(compare_with = "compare_cards_unordered"),
    Hash(hash_with = "hash_cards_unordered")
  )]
  pub hand: ArrayVec<[SingleCard; 10]>,
  #[derivative(
    PartialEq(compare_with = "compare_cards_unordered"),
    Hash(hash_with = "hash_cards_unordered")
  )]
  pub limbo: Vec<SingleCard>,
  pub card_in_play: Option<SingleCard>,
  pub player: Player,
  pub monsters: ArrayVec<[Monster; MAX_MONSTERS]>,
  pub turn_number: i32,
  pub turn_has_ended: bool,

  pub fresh_subaction_queue: Vec<DynAction>,
  pub stale_subaction_stack: Vec<DynAction>,
  pub actions: VecDeque<DynAction>,
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SingleCard {
  pub misc: i32,
  pub cost: i32,
  pub upgrades: i32,
  pub card_info: Arc<CardInfo>,
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Debug)]
pub enum CardType {
  Attack,
  Skill,
  Power,
  Status,
  Curse,
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Debug)]
pub enum Rarity {
  Common,
  Uncommon,
  Rare,
  Basic,
  Special,
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Debug)]
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
  fn default() -> CardInfo {
    CardInfo {
      id: CardId::Injury,
      card_type: CardType::Curse,
      rarity: Rarity::Special,
      normal_cost: UNPLAYABLE,
      upgraded_cost: -3,
      ethereal: false,
      has_target: false,
      exhausts: false,
    }
  }
}

#[derive(Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Creature {
  pub hitpoints: i32,
  pub max_hitpoints: i32,
  pub block: i32,
  pub powers: Vec<Power>,
}

#[derive(Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Debug)]
pub struct Player {
  pub creature: Creature,
  pub energy: i32,
}

#[derive(Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Debug)]
pub struct Monster {
  pub monster_id: MonsterId,
  pub innate_damage_amount: Option<i32>,
  //pub misc: i32,
  pub ascension: i32,
  pub creature: Creature,
  pub move_history: Vec<i32>,
  pub gone: bool,
}

#[derive(Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Power {
  pub power_id: PowerId,
  pub amount: i32,
  #[serde(default)]
  pub damage: i32,
  pub card: Option<SingleCard>,
  #[serde(default)]
  pub misc: i32,
  pub just_applied: bool,
}

impl Default for Power {
  fn default() -> Power {
    Power {
      power_id: PowerId::Unknown,
      amount: 0,
      damage: 0,
      card: None,
      misc: 0,
      just_applied: false,
    }
  }
}

/*#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Relic {
  name: String,
  id: String,
  counter: i32,
}*/

impl CombatState {
  pub fn from_communication_mod(
    observed: &communication::GameState,
    previous: Option<&CombatState>,
  ) -> Option<CombatState> {
  
    let combat = observed.combat_state.as_ref()?;
    let mut draw_pile: Vec<SingleCard> = combat.draw_pile.iter().map(From::from).collect();
    // explicitly sort, partly to make sure my AI doesn't accidentally cheat
    draw_pile.sort();
    
    let mut result = CombatState {
      draw_pile,
      discard_pile: combat.discard_pile.iter().map(From::from).collect(),
      exhaust_pile: combat.exhaust_pile.iter().map(From::from).collect(),
      hand: combat.hand.iter().map(From::from).collect(),
      limbo: combat.limbo.iter().map(From::from).collect(),
      card_in_play: combat.card_in_play.as_ref().map(From::from),
      fresh_subaction_queue: Vec::new(),
      stale_subaction_stack: Vec::new(),
      actions: VecDeque::new(),
      player: Player::from_communication_mod(&combat.player, & observed.relics),
      turn_number: combat.turn,
      turn_has_ended: false,
      monsters: combat
        .monsters
        .iter()
        .map(|monster| {
          let mut move_history = vec![monster.move_id];
          if let Some(previous) = monster.last_move_id {
            move_history.insert(0, previous);
          }
          if let Some(previous) = monster.second_last_move_id {
            move_history.insert(0, previous);
          }
          let innate_damage_amount = if monster.move_base_damage > 0 {
            Some(monster.move_base_damage)
          } else {
            None
          };
          Monster {
            monster_id: MonsterId::from(&*monster.id),
            ascension: observed.ascension_level,
            move_history,
            innate_damage_amount,
            creature: Creature {
              hitpoints: monster.current_hp,
              max_hitpoints: monster.max_hp,
              block: monster.block,
              powers: monster.powers.iter().map(From::from).collect(),
            },
            gone: monster.is_gone,
          }
        })
        .collect(),
    };

    if let Some(previous) = previous {
      for (monster, new_version) in previous.monsters.iter().zip(&mut result.monsters) {
        if new_version.innate_damage_amount.is_none() {
          new_version.innate_damage_amount = monster.innate_damage_amount;
        } /* else {
            if new_version.innate_damage_amount != monster.innate_damage_amount {
              eprintln!(
                " Unexpected change in innate damage amount: {:?} ",
                (monster, new_version)
              );
            }
          }*/
      }
    }
    Some(result)
  }
}

impl From<&communication::Card> for SingleCard {
  fn from(card: &communication::Card) -> SingleCard {
    SingleCard {
      misc: card.misc,
      cost: card.cost,
      upgrades: card.upgrades,
      card_info: Arc::new(CardInfo::from(CardId::from(&*card.id))),
    }
  }
}

impl From<&communication::Power> for Power {
  fn from(power: &communication::Power) -> Power {
    Power {
      power_id: PowerId::from(&*power.id),
      amount: power.amount,
      damage: power.damage,
      card: power.card.as_ref().map(From::from),
      misc: power.misc,
      just_applied: power.just_applied,
    }
  }
}

impl From<&communication::Relic> for Power {
  fn from(relic: &communication::Relic) -> Power {
    Power {
      power_id: PowerId::from(&*relic.id),
      amount: relic.counter,
      damage: 0,
      card: None,
      misc: 0,
      just_applied: false,
    }
  }
}

impl Player {
  fn from_communication_mod(player: &communication::Player, relics: & [communication::Relic]) -> Player {
    Player {
      energy: player.energy,
      creature: Creature {
        hitpoints: player.current_hp,
        max_hitpoints: player.max_hp,
        block: player.block,
        powers: relics.iter().map(Power::from).filter(|p| p.power_id != PowerId::Unknown).chain (player.powers.iter().map (From::from)).collect(),
      },
    }
  }
}

impl SingleCard {
  pub fn start_combat_cost(&self) -> i32 {
    if self.upgrades > 0 {
      self.card_info.upgraded_cost
    } else {
      self.card_info.normal_cost
    }
  }

  pub fn create(id: CardId) -> SingleCard {
    let info = CardInfo::from(id);
    SingleCard {
      misc: 0,
      cost: info.normal_cost,
      upgrades: 0,
      card_info: Arc::new(info),
    }
  }
  
  pub fn upgrade (&mut self) {
    if self.upgrades == 0 {self.upgrades = 1;}
  }
}

impl Debug for Creature {
  fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
    write!(f, "{}/{}", self.hitpoints, self.max_hitpoints)?;
    if self.block > 0 {
      write!(f, "(+{})", self.block)?;
    }
    for power in &self.powers {
      write!(f, " {:?}", power)?;
    }
    Ok(())
  }
}

impl Debug for Power {
  fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
    write!(f, "{:?}", self.power_id)?;
    if self.amount != 0 {
      write!(f, "{}", self.amount)?;
    }
    if self.just_applied {
      write!(f, "j")?;
    }
    Ok(())
  }
}

impl Debug for SingleCard {
  fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
    write!(f, "{:?}", self.card_info.id)?;

    if self.upgrades == 1 {
      write!(f, "+")?;
    } else if self.upgrades != 0 {
      write!(f, "+{}", self.upgrades)?;
    }

    if self.misc != 0 {
      write!(f, "?{}", self.misc)?;
    }
    if self.cost != self.start_combat_cost() {
      write!(f, "({})", self.cost)?;
    }
    Ok(())
  }
}

/*impl Debug for Choice {
  fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
    match self {
      Choice::EndTurn => write!(f, "EndTurn"),
      Choice::PlayCard (card, target) => {
        if card.card_info.has_target {
          write!(f, "{:?}@{}", card, target)
        }
        else {
          write!(f, "{:?}", card)
        }
      }
    }
  }
}*/
