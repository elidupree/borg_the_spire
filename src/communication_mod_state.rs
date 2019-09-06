use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct CommunicationState {
  pub available_commands: Vec<String>,
  pub ready_for_command: bool,
  pub in_game: bool,
  pub game_state: Option<GameState>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct GameState {
  pub screen_name: String,
  pub is_screen_up: bool,
  pub screen_type: String,
  pub screen_state: Value,
  pub room_phase: String,
  pub action_phase: String,
  pub room_type: String,
  pub current_hp: i32,
  pub max_hp: i32,
  pub floor: i32,
  pub act: i32,
  pub act_boss: String,
  pub gold: i32,
  pub seed: i64,
  pub class: String,
  pub ascension_level: i32,
  pub relics: Vec<Relic>,
  pub deck: Vec<Card>,
  pub potions: Value,
  pub map: Value,
  pub current_action: Option<Value>,
  pub combat_state: Option<CombatState>,
  pub choice_list: Option<Value>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct CombatState {
  pub draw_pile: Vec<Card>,
  pub discard_pile: Vec<Card>,
  pub exhaust_pile: Vec<Card>,
  pub hand: Vec<Card>,
  pub limbo: Vec<Card>,
  pub card_in_play: Option <Card>,
  pub cards_discarded_this_turn: i32,
  pub turn: i32,
  pub player: Player,
  pub monsters: Vec<Monster>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Card {
  pub name: String,
  pub uuid: String,
  #[serde(default)]
  pub misc: i32,
  #[serde(default)]
  pub is_playable: bool,
  pub cost: i32,
  pub upgrades: i32,
  pub id: String,
  #[serde(rename = "type")]
  pub card_type: String,
  pub rarity: String,
  pub has_target: bool,
  pub exhausts: bool,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Player {
  pub current_hp: i32,
  pub max_hp: i32,
  pub block: i32,
  pub powers: Vec<Power>,
  pub energy: i32,
  pub orbs: Vec<Value>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Monster {
  pub name: String,
  pub id: String,
  pub current_hp: i32,
  pub max_hp: i32,
  pub block: i32,
  pub intent: String,
  #[serde(default)]
  pub move_id: i32,
  pub last_move_id: Option<i32>,
  pub second_last_move_id: Option<i32>,
  #[serde(default)]
  pub move_base_damage: i32,
  #[serde(default)]
  pub move_adjusted_damage: i32,
  #[serde(default)]
  pub move_hits: i32,
  pub half_dead: bool,
  pub is_gone: bool,
  pub powers: Vec<Power>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Power {
  pub id: String,
  pub name: String,
  pub amount: i32,
  #[serde(default)]
  pub damage: i32,
  pub card: Option <Card>,
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
