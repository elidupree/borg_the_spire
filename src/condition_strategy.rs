use crate::actions::{PlayCard, UsePotion};
use crate::ai_utils::{card_play_stats, CardPlayStats, Strategy};
use crate::simulation::{Choice, MonsterIndex};
use crate::simulation_state::monsters::MAX_INTENTS;
use crate::simulation_state::{
  CardId, CardType, CombatState, Creature, Monster, MonsterId, PowerId, SingleCard, MAX_MONSTERS,
};
use array_ext::Array;
use ordered_float::OrderedFloat;
use rand::seq::{IteratorRandom, SliceRandom};
use rand::Rng;
use rand_distr::{Poisson, StandardNormal};
use std::collections::HashSet;

#[derive(Clone, Debug, Default)]
pub struct Rule {
  pub conditions: Vec<Condition>,
  pub flat_reward: f64,
  pub block_per_energy_reward: f64,
  pub unblocked_damage_per_energy_rewards: [f64; MAX_MONSTERS],
}
#[derive(Clone, Debug)]
pub struct Condition {
  pub inverted: bool,
  pub kind: ConditionKind,
}
#[derive(Clone, Debug)]
pub struct WhichMonster {
  pub id: MonsterId,
  pub which_of_this_id: usize,
}
#[derive(Clone, Debug)]
pub enum WhichCreature {
  Player,
  Monster(WhichMonster),
}
impl WhichMonster {
  pub fn index(&self, state: &CombatState) -> Option<MonsterIndex> {
    self.get_with_index(state).map(|(i, _)| i)
  }
  pub fn get<'a>(&self, state: &'a CombatState) -> Option<&'a Monster> {
    self.get_with_index(state).map(|(_, m)| m)
  }
  pub fn get_with_index<'a>(&self, state: &'a CombatState) -> Option<(MonsterIndex, &'a Monster)> {
    state
      .monsters
      .iter()
      .enumerate()
      .filter(|(_, m)| m.monster_id == self.id)
      .nth(self.which_of_this_id)
      .filter(|(_, m)| !m.gone)
  }
  pub fn random(state: &CombatState, rng: &mut impl Rng) -> WhichMonster {
    let (index, monster) = state
      .monsters
      .iter()
      .enumerate()
      .filter(|(_, m)| !m.gone)
      .choose(rng)
      .unwrap();
    let id = monster.monster_id;
    WhichMonster {
      id,
      which_of_this_id: state.monsters[..index]
        .iter()
        .filter(|m| m.monster_id == id)
        .count(),
    }
  }
}
impl WhichCreature {
  pub fn get<'a>(&self, state: &'a CombatState) -> Option<&'a Creature> {
    match self {
      WhichCreature::Player => Some(&state.player.creature),
      WhichCreature::Monster(monster) => monster.get(state).map(|m| &m.creature),
    }
  }
}
#[derive(Clone, Debug)]
pub enum ConditionKind {
  EndTurn,
  PlayCard,
  PlayCardType(CardType),
  PlayCardId(CardId),
  UsePotion,
  UsePotionId(CardId),
  TargetsMonster(WhichMonster),
  Upgraded,
  NumericPropertyGt {
    threshold: i32,
    property: NumericProperty,
  },
  MonsterIntent {
    monster: WhichMonster,
    intent_included: [bool; MAX_INTENTS],
  },
  HasPower {
    creature: WhichCreature,
    power: PowerId,
  },
}
#[derive(Clone, Debug)]
pub enum NumericProperty {
  TurnNumber,
  Energy,
  CreatureHitpoints(WhichCreature),
  UnblockedDamageToMonster(WhichMonster),
  IncomingUnblockedDamage,
  PowerAmount {
    creature: WhichCreature,
    power: PowerId,
  },
}
pub struct EvaluationData {
  pub incoming_damage: i32,
  pub choices: Vec<ChoiceEvaluationData>,
}
pub struct ChoiceEvaluationData {
  pub choice: Choice,
  pub stats: CardPlayStats,
}
pub struct ChoiceEvaluationContext<'a> {
  pub global: &'a EvaluationData,
  pub choice: &'a ChoiceEvaluationData,
}
pub struct EvaluatedPriorities {
  pub priorities: Vec<f64>,
}
impl Rule {
  pub fn applied_priority(&self, state: &CombatState, context: &ChoiceEvaluationContext) -> f64 {
    let mut result = 0.0;
    if self.conditions.iter().all(|c| c.evaluate(state, context)) {
      self.flat_reward;
      result += self.flat_reward;
      if let Choice::PlayCard(choice) = &context.choice.choice {
        let energy = (choice.card.cost_in_practice(state) as f64).min(0.5);
        if self.block_per_energy_reward != 0.0 {
          result +=
            self.block_per_energy_reward * context.choice.stats.block_amount as f64 / energy;
        }
        for (index, damage) in context.choice.stats.damage.iter().enumerate() {
          result += self.unblocked_damage_per_energy_rewards[index]
            * (damage - state.monsters[index].creature.block as f64).max(0.0)
            / energy;
        }
      }
    }
    result
  }
}
impl EvaluationData {
  pub fn contexts(&self) -> impl Iterator<Item = ChoiceEvaluationContext> {
    self
      .choices
      .iter()
      .map(move |choice| ChoiceEvaluationContext {
        global: self,
        choice,
      })
  }
  pub fn new(state: &CombatState) -> Self {
    let legal_choices = state.legal_choices();
    let incoming_damage =
      (state.total_monster_attack_intent_damage() - state.player.creature.block).max(0);
    let choices = legal_choices
      .into_iter()
      .map(|choice| {
        let stats = match &choice {
          Choice::PlayCard(PlayCard { card, target }) => card_play_stats(state, card, *target),
          &Choice::UsePotion(UsePotion {
            potion_info,
            target,
          }) => card_play_stats(
            state,
            &SingleCard {
              misc: 0,
              cost: 0,
              upgrades: 0,
              card_info: potion_info,
            },
            target,
          ),
          _ => CardPlayStats::default(),
        };
        ChoiceEvaluationData { choice, stats }
      })
      .collect();
    EvaluationData {
      incoming_damage,
      choices,
    }
  }
}
impl EvaluatedPriorities {
  pub fn evaluated<'a>(
    rules: impl IntoIterator<Item = &'a Rule>,
    state: &CombatState,
    data: &EvaluationData,
  ) -> EvaluatedPriorities {
    let mut result = EvaluatedPriorities {
      priorities: data.choices.iter().map(|_| 0.0).collect(),
    };
    for rule in rules {
      result.apply_rule(rule, state, data);
    }
    result
  }
  pub fn apply_rule(&mut self, rule: &Rule, state: &CombatState, data: &EvaluationData) {
    for (priority, context) in self.priorities.iter_mut().zip(data.contexts()) {
      *priority += rule.applied_priority(state, &context);
    }
  }
  pub fn best_index(&self) -> usize {
    self
      .priorities
      .iter()
      .enumerate()
      .max_by_key(|&(_, &p)| OrderedFloat(p))
      .unwrap()
      .0
  }
  pub fn best_index_with_extra_rule(
    &self,
    rule: &Rule,
    state: &CombatState,
    data: &EvaluationData,
  ) -> usize {
    self
      .priorities
      .iter()
      .zip(data.contexts())
      .enumerate()
      .max_by_key(|(_, (&p, context))| OrderedFloat(p + rule.applied_priority(state, &context)))
      .unwrap()
      .0
  }
}
impl Condition {
  pub fn evaluate(&self, state: &CombatState, context: &ChoiceEvaluationContext) -> bool {
    use ConditionKind::*;
    self.inverted
      ^ match &self.kind {
        EndTurn => {
          matches!(&context.choice.choice, Choice::EndTurn(_))
        }
        PlayCard => {
          matches!(&context.choice.choice, Choice::PlayCard(_))
        }
        PlayCardType(card_type) => {
          matches!(
            &context.choice.choice,
            Choice::PlayCard(p) if p.card.card_info.card_type == *card_type
          )
        }
        PlayCardId(card_id) => {
          matches!(
            &context.choice.choice,
            Choice::PlayCard(p) if p.card.card_info.id == *card_id
          )
        }
        UsePotion => {
          matches!(&context.choice.choice, Choice::UsePotion(_))
        }
        UsePotionId(card_id) => {
          matches!(
            &context.choice.choice,
            Choice::UsePotion(p) if p.potion_info.id == *card_id
          )
        }
        TargetsMonster(monster) => {
          matches!(
            &context.choice.choice,
            Choice::PlayCard(p) if p.card.card_info.has_target && Some(p.target) == monster.index(state)
          ) || matches!(
            &context.choice.choice,
            Choice::UsePotion(p) if p.potion_info.has_target && Some(p.target) == monster.index(state)
          )
        }
        Upgraded => {
          matches!(
            &context.choice.choice,
            Choice::PlayCard(p) if p.card.upgrades > 0
          )
        }
        NumericPropertyGt {
          threshold,
          property: numeric,
        } => numeric.evaluate(state, context) > *threshold,
        MonsterIntent {
          monster,
          intent_included,
        } => {
          if let Some(monster) = monster.get(state) {
            intent_included[monster.intent() as usize]
          } else {
            false
          }
        }
        HasPower { creature, power } => {
          if let Some(creature) = creature.get(state) {
            creature.has_power(*power)
          } else {
            false
          }
        }
      }
  }
  pub fn random_generally_relevant_choice_distinguisher<R: Rng>(
    state: &CombatState,
    rng: &mut R,
  ) -> Condition {
    use ConditionKind::*;
    let mut options: Vec<ConditionKind> = vec![
      PlayCard,
      EndTurn,
      PlayCardType(CardType::Attack),
      PlayCardType(CardType::Skill),
      PlayCardType(CardType::Power),
      TargetsMonster(WhichMonster::random(state, rng)),
    ];

    for card_id in state
      .hand
      .iter()
      .chain(&state.draw_pile)
      .chain(&state.discard_pile)
      .map(|c| c.card_info.id)
      .collect::<HashSet<_>>()
    {
      options.push(PlayCardId(card_id));
    }

    for potion in &state.potions {
      options.push(UsePotionId(potion.id));
    }

    Condition {
      inverted: rng.gen(),
      kind: options.choose(rng).unwrap().clone(),
    }
  }
  pub fn random_generally_relevant_state_distinguisher<R: Rng>(
    state: &CombatState,
    rng: &mut R,
  ) -> Condition {
    use ConditionKind::*;
    use NumericProperty::*;
    let options: Vec<ConditionKind> = vec![
      MonsterIntent {
        monster: WhichMonster::random(state, rng),
        intent_included: Array::from_fn(|_| rng.gen()),
      },
      NumericPropertyGt {
        threshold: rng.sample(Poisson::new(1.0f64).unwrap()) as i32 + 1,
        property: TurnNumber,
      },
      NumericPropertyGt {
        threshold: rng.gen_range(1..(state.player.creature.hitpoints - 1).max(2)),
        property: CreatureHitpoints(WhichCreature::Player),
      },
      {
        let monster = WhichMonster::random(state, rng);
        NumericPropertyGt {
          threshold: rng.gen_range(1..(monster.get(state).unwrap().creature.hitpoints - 1).max(2)),
          property: CreatureHitpoints(WhichCreature::Monster(monster)),
        }
      },
      NumericPropertyGt {
        threshold: rng.gen_range(0..30),
        property: IncomingUnblockedDamage,
      },
      NumericPropertyGt {
        threshold: rng.gen_range(0..=3),
        property: Energy,
      },
    ];

    Condition {
      inverted: rng.gen(),
      kind: options.choose(rng).unwrap().clone(),
    }
  }
  pub fn random_generally_relevant<R: Rng>(state: &CombatState, rng: &mut R) -> Condition {
    if rng.gen() {
      Self::random_generally_relevant_choice_distinguisher(state, rng)
    } else {
      Self::random_generally_relevant_state_distinguisher(state, rng)
    }
  }
}
impl From<ConditionKind> for Condition {
  fn from(kind: ConditionKind) -> Self {
    Condition {
      inverted: false,
      kind,
    }
  }
}
impl NumericProperty {
  pub fn evaluate(&self, state: &CombatState, context: &ChoiceEvaluationContext) -> i32 {
    use NumericProperty::*;
    match self {
      TurnNumber => state.turn_number,
      Energy => state.player.energy,
      CreatureHitpoints(creature) => {
        if let Some(creature) = creature.get(state) {
          creature.hitpoints
        } else {
          0
        }
      }
      IncomingUnblockedDamage => {
        (context.global.incoming_damage - state.player.creature.block).max(0)
      }
      PowerAmount { creature, power } => {
        if let Some(creature) = creature.get(state) {
          creature.power_amount(*power)
        } else {
          0
        }
      }
      UnblockedDamageToMonster(monster) => {
        if let Some((index, monster)) = monster.get_with_index(state) {
          (context.choice.stats.damage[index] as i32 - monster.creature.block).max(0)
        } else {
          0
        }
      }
    }
  }
}

#[derive(Clone, Debug)]
pub struct ConditionStrategy {
  pub annotation: String,
  pub rules: Vec<Rule>,
}

impl Strategy for ConditionStrategy {
  fn choose_choice(&self, state: &CombatState) -> Vec<Choice> {
    let data = EvaluationData::new(state);
    let priorities = EvaluatedPriorities::evaluated(&self.rules, state, &data);
    vec![data.choices[priorities.best_index()].choice.clone()]
  }
}
impl ConditionStrategy {
  // not required to be able to generate all POSSIBLE strategies,
  // just trying to create ones that are well spread over the space of plausibly good strategies,
  // and might be able to hill-climb to a nearby optimum.
  pub fn fresh_distinctive_candidate(state: &CombatState, rng: &mut impl Rng) -> ConditionStrategy {
    let mut rules = vec![
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
      Rule {
        conditions: vec![Condition::from(ConditionKind::PlayCard)],
        flat_reward: 1.0,
        ..Default::default()
      },
      Rule {
        conditions: vec![Condition::from(ConditionKind::UsePotion)],
        flat_reward: -0.4,
        ..Default::default()
      },
      Rule {
        conditions: vec![Condition::from(ConditionKind::Upgraded)],
        flat_reward: 0.1,
        ..Default::default()
      },
    ];
    for card_id in state
      .hand
      .iter()
      .chain(&state.draw_pile)
      .chain(&state.discard_pile)
      .map(|c| c.card_info.id)
      .collect::<HashSet<_>>()
    {
      rules.push(Rule {
        conditions: vec![Condition::from(ConditionKind::PlayCardId(card_id))],
        flat_reward: rng.sample(StandardNormal),
        ..Default::default()
      })
    }
    ConditionStrategy {
      annotation: "fresh_distinctive_candidate".to_string(),
      rules,
    }
  }
}
