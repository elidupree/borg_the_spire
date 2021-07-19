use crate::actions::PlayCard;
use crate::ai_utils::{card_play_stats, CardPlayStats, Strategy};
use crate::simulation::{Choice, CreatureIndex, MonsterIndex};
use crate::simulation_state::monsters::MAX_INTENTS;
use crate::simulation_state::{CardId, CombatState, PowerId, MAX_MONSTERS};
use array_ext::Array;
use enum_map::EnumMap;
use ordered_float::OrderedFloat;
use rand::Rng;

#[derive(Debug, Default)]
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
      if let Choice::PlayCard(choice) = choice {
        let energy = (choice.card.cost_in_practice(state) as f64).min(0.5);
        if self.block_per_energy_reward != 0.0 {
          result += self.block_per_energy_reward * context.stats.block_amount as f64 / energy;
        }
        for (index, damage) in context.stats.damage.iter().enumerate() {
          result += self.unblocked_damage_per_energy_rewards[index]
            * (damage - state.monsters[index].creature.block as f64).max(0.0)
            / energy;
        }
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
  pub fn random_generally_relevant(state: &CombatState, rng: &mut impl Rng) -> Condition {
    use Condition::*;
    use NumericProperty::*;
    match rng.gen_range(0..=4) {
      0 => MonsterIntent {
        monster_index: rng.gen_range(0..state.monsters.len()),
        intent_included: Array::from_fn(|_| rng.gen()),
        gone_included: rng.gen(),
      },
      1 => NumericThreshold {
        threshold: rng.gen_range(1..5).min(rng.gen_range(1..5)),
        gt: rng.gen(),
        property: TurnNumber,
      },
      2 => NumericThreshold {
        threshold: rng.gen_range(1..(state.player.creature.hitpoints - 1).max(2)),
        gt: rng.gen(),
        property: CreatureHitpoints(CreatureIndex::Player),
      },
      3 => {
        let monster_index = rng.gen_range(0..state.monsters.len());
        NumericThreshold {
          threshold: rng
            .gen_range(1..(state.monsters[monster_index].creature.hitpoints - 1).max(2)),
          gt: rng.gen(),
          property: CreatureHitpoints(CreatureIndex::Monster(monster_index)),
        }
      }
      4 => NumericThreshold {
        threshold: rng.gen_range(0..30),
        gt: rng.gen(),
        property: IncomingDamage,
      },
      _ => unreachable!(),
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

  // not required to be able to generate all POSSIBLE strategies,
  // just trying to create ones that are well spread over the space of plausibly good strategies,
  // and might be able to hill-climb to a nearby optimum.
  pub fn fresh_distinctive_candidate(state: &CombatState, rng: &mut impl Rng) -> ConditionStrategy {
    ConditionStrategy {
      play_card_global_rules: vec![
        Rule {
          conditions: vec![],
          block_per_energy_reward: rng.gen::<f64>() * 0.1,
          ..Default::default()
        },
        Rule {
          conditions: vec![],
          unblocked_damage_per_energy_rewards: Array::from_fn(|_| rng.gen::<f64>() * 0.05),
          ..Default::default()
        },
      ],
      play_specific_card_rules: EnumMap::from(|_| {
        let mut rules = vec![Rule {
          conditions: vec![],
          flat_reward: rng.gen::<f64>() - 0.5,
          ..Default::default()
        }];
        for _ in 0..rng.gen_range(0..=2) {
          rules.push(Rule {
            conditions: vec![Condition::random_generally_relevant(state, rng)],
            flat_reward: rng.gen::<f64>() - 0.5,
            ..Default::default()
          })
        }
        rules
      }),
    }
  }
}
