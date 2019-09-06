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
pub struct CombatState {}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Card {
  name: String,
  uuid: String,
  #[serde(default)]
  misc: i32,
  #[serde(default)]
  is_playable: bool,
  cost: i32,
  upgrades: i32,
  id: String,
  #[serde(rename = "type")]
  card_type: String,
  rarity: String,
  has_target: bool,
  exhausts: bool,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Relic {
  name: String,
  id: String,
  counter: i32,
}
