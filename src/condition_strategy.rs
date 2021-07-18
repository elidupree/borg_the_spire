use crate::actions::PlayCard;
use crate::ai_utils::{card_play_stats, CardPlayStats, Strategy};
use crate::simulation::{Choice, CreatureIndex, MonsterIndex};
use crate::simulation_state::monsters::MAX_INTENTS;
use crate::simulation_state::{CardId, CombatState, PowerId, MAX_MONSTERS};
use enum_map::EnumMap;
use ordered_float::OrderedFloat;

#[derive(Debug)]
pub struct Rule {
  pub conditions: Vec<Condition>,
  pub flat_reward: f64,
  pub block_per_energy_reward: f64,
  pub unblocked_damage_per_energy_rewards: [f64; MAX_MONSTERS],
}
#[derive(Debug)]
pub enum Condition {
  NumericThreshold {
    threshold: i32,
    gt: bool,
    property: NumericProperty,
  },
  MonsterIntent {
    monster_index: MonsterIndex,
    intent_included: [bool; MAX_INTENTS],
    gone_included: bool,
  },
  HasPower {
    creature_index: CreatureIndex,
    power: PowerId,
    inverted: bool,
  },
}
#[derive(Debug)]
pub enum NumericProperty {
  TurnNumber,
  CreatureHitpoints(CreatureIndex),
  UnblockedDamageToMonster(MonsterIndex),
  IncomingDamage,
  PowerAmount {
    creature_index: CreatureIndex,
    power: PowerId,
  },
}
pub struct EvaluationContext {
  incoming_damage: i32,
}
pub struct ChoiceEvaluationContext<'a> {
  global: &'a EvaluationContext,
  stats: CardPlayStats,
}
impl Rule {
  pub fn applied_priority(
    &self,
    state: &CombatState,
    choice: &Choice,
    context: &ChoiceEvaluationContext,
  ) -> f64 {
    let mut result = 0.0;
    if self
      .conditions
      .iter()
      .all(|c| c.evaluate(state, choice, context))
    {
      self.flat_reward;
      result += self.flat_reward;
      if self.block_per_energy_reward != 0.0 {
        result += self.block_per_energy_reward * 1.0;
      }
    }
    result
  }
}
impl Condition {
  pub fn evaluate(
    &self,
    state: &CombatState,
    choice: &Choice,
    context: &ChoiceEvaluationContext,
  ) -> bool {
    use Condition::*;
    match self {
      NumericThreshold {
        threshold,
        gt,
        property: numeric,
      } => (numeric.evaluate(state, choice, context) > *threshold) == *gt,
      MonsterIntent {
        monster_index,
        intent_included,
        gone_included,
      } => {
        let monster = &state.monsters[*monster_index];
        if monster.gone {
          *gone_included
        } else {
          intent_included[monster.intent() as usize]
        }
      }
      HasPower {
        creature_index,
        power,
        inverted,
      } => {
        // TODO: `gone` handling
        let creature = state.get_creature(*creature_index);
        creature.has_power(*power) != *inverted
      }
    }
  }
}
impl NumericProperty {
  pub fn evaluate(
    &self,
    state: &CombatState,
    _choice: &Choice,
    context: &ChoiceEvaluationContext,
  ) -> i32 {
    use NumericProperty::*;
    match self {
      TurnNumber => state.turn_number,
      &CreatureHitpoints(creature_index) => {
        // TODO: `gone` handling
        let creature = state.get_creature(creature_index);
        creature.hitpoints
      }
      IncomingDamage => context.global.incoming_damage,
      &PowerAmount {
        creature_index,
        power,
      } => {
        // TODO: `gone` handling
        let creature = state.get_creature(creature_index);
        creature.power_amount(power)
      }
      &UnblockedDamageToMonster(monster_index) => (context.stats.damage[monster_index] as i32
        - state.monsters[monster_index].creature.block)
        .max(0),
    }
  }
}

#[derive(Debug)]
pub struct ConditionStrategy {
  play_card_global_rules: Vec<Rule>,
  play_specific_card_rules: EnumMap<CardId, Vec<Rule>>,
}

impl Strategy for ConditionStrategy {
  fn choose_choice(&self, state: &CombatState) -> Vec<Choice> {
    let legal_choices = state.legal_choices();

    let incoming_damage =
      (state.total_monster_attack_intent_damage() - state.player.creature.block).max(0);
    let context = EvaluationContext { incoming_damage };

    vec![legal_choices
      .into_iter()
      .max_by_key(|choice| OrderedFloat(self.evaluate(state, choice, &context)))
      .unwrap()]
  }
}
impl ConditionStrategy {
  pub fn evaluate(&self, state: &CombatState, choice: &Choice, context: &EvaluationContext) -> f64 {
    match choice {
      Choice::EndTurn(_) => 0.0,
      Choice::PlayCard(PlayCard { card, target }) => {
        let context = ChoiceEvaluationContext {
          global: context,
          stats: card_play_stats(state, card, *target),
        };
        self
          .play_card_global_rules
          .iter()
          .chain(&self.play_specific_card_rules[card.card_info.id])
          .map(|r| r.applied_priority(state, choice, &context))
          .sum::<f64>()
      }
      _ => 0.0,
    }
  }
}
