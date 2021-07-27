use super::*;

intent! {
  pub enum ByrdIntent {
    6: Caw,
    1: Peck,
    3: Swoop,
    2: Fly,
    5: Headbutt,
    4: Stunned,
  }
}
impl MonsterBehavior for Byrd {
  type Intent = ByrdIntent;
  fn make_intent_distribution(context: &mut IntentChoiceContext) {
    use ByrdIntent::*;
    if context.first_move() {
      context.always(split(0.375, Caw, Peck));
    } else if context.monster().creature.has_power(PowerId::Flight) {
      context.if_num_lt(
        50,
        context.with_max_repeats(Repeats(2), Peck, split(0.4, Swoop, Caw)),
      );
      context.if_num_lt(
        70,
        context.with_max_repeats(Repeats(1), Swoop, split(0.375, Caw, Peck)),
      );
      context.else_num(context.with_max_repeats(Repeats(1), Caw, split(0.2857, Swoop, Peck)));
    } else {
      context.always(context.with_max_repeats(Repeats(1), Headbutt, Fly));
    }
  }
  fn intent_effects(context: &mut impl IntentEffectsContext) {
    use ByrdIntent::*;
    match context.intent::<Self::Intent>() {
      Peck => {
        for _ in 0..context.with_ascension(Ascension(2), 6, 5) {
          context.attack(1);
        }
      }
      Headbutt => context.attack(3),
      Fly => context.power_self(PowerId::Flight, context.with_ascension(Ascension(17), 4, 3)),
      Caw => context.power_self(PowerId::Strength, 1),
      Swoop => context.attack(context.with_ascension(Ascension(2), 14, 12)),
      Stunned => {}
    }
  }
}

intent! {
  pub enum SphericGuardianIntent {
    1: Slam,
    2: Activate,
    3: Harden,
    4: AttackDebuff,
  }
}
impl MonsterBehavior for SphericGuardian {
  type Intent = SphericGuardianIntent;
  fn make_intent_distribution(context: &mut IntentChoiceContext) {
    use SphericGuardianIntent::*;
    if context.first_move() {
      context.always(Activate);
    } else if context.second_move() {
      context.always(AttackDebuff);
    } else {
      context.always(context.with_max_repeats(Repeats(1), Slam, Harden));
    }
  }
  fn intent_effects(context: &mut impl IntentEffectsContext) {
    use SphericGuardianIntent::*;
    match context.intent::<Self::Intent>() {
      Slam => {
        for _ in 0..2 {
          context.attack(context.with_ascension(Ascension(2), 11, 10));
        }
      }
      Activate => context.block(context.with_ascension(Ascension(17), 35, 25)),
      Harden => {
        context.attack(context.with_ascension(Ascension(2), 11, 10));
        context.block(15);
      }
      AttackDebuff => {
        context.power_player(PowerId::Frail, 5);
        context.attack(context.with_ascension(Ascension(2), 11, 10));
      }
    }
  }
}

intent! {
  pub enum TaskmasterIntent {
    2: ScouringWhip,
  }
}
impl MonsterBehavior for Taskmaster {
  type Intent = TaskmasterIntent;
  fn make_intent_distribution(context: &mut IntentChoiceContext) {
    use TaskmasterIntent::*;
    context.always(ScouringWhip);
  }
  fn intent_effects(context: &mut impl IntentEffectsContext) {
    use TaskmasterIntent::*;
    match context.intent::<Self::Intent>() {
      ScouringWhip => {
        context.attack(7);
        context.discard_status(
          CardId::Wound,
          context.with_ascensions(Ascension(18), 3, Ascension(3), 2, 1),
        );
        if context.ascension() >= 18 {
          context.power_self(PowerId::Strength, 1);
        }
      }
    }
  }
}

intent! {
  pub enum GremlinLeaderIntent {
    2: Rally,
    3: Encourage,
    4: Stab,
  }
}
impl MonsterBehavior for GremlinLeader {
  type Intent = GremlinLeaderIntent;
  fn make_intent_distribution(context: &mut IntentChoiceContext) {
    use GremlinLeaderIntent::*;
    let num_alive_gremlins = context
      .state()
      .monsters
      .iter()
      .enumerate()
      .filter(|&(i, m)| !m.gone && i != context.monster_index())
      .count();
    if num_alive_gremlins == 0 {
      context.if_num_lt(75, context.with_max_repeats(Repeats(1), Rally, Stab));
      context.else_num(context.with_max_repeats(Repeats(1), Stab, Rally));
    } else if num_alive_gremlins < 2 {
      let last = context.last_intent::<GremlinLeaderIntent>().unwrap();
      context.always(match last {
        Stab => split(0.625, Rally, Encourage),
        Rally => unreachable!(),
        Encourage => split(0.5, Rally, Stab),
      });
    } else {
      context.if_num_lt(66, context.with_max_repeats(Repeats(1), Encourage, Stab));
      context.else_num(context.with_max_repeats(Repeats(1), Stab, Encourage));
    }
  }
  fn intent_effects(context: &mut impl IntentEffectsContext) {
    use GremlinLeaderIntent::*;
    match context.intent::<Self::Intent>() {
      Stab => {
        for _ in 0..3 {
          context.attack(6);
        }
      }
      Rally => {
        context.action(SummonGremlinAction);
        context.action(SummonGremlinAction);
      }
      Encourage => {
        let source = context.creature_index();
        let strength_amount = context.with_ascensions(Ascension(18), 5, Ascension(3), 4, 3);
        let block_amount = context.with_ascension(Ascension(18), 10, 6);
        for i in 0..context.state().monsters.len() {
          context.action(ApplyPowerAction {
            source,
            target: CreatureIndex::Monster(i),
            power_id: PowerId::Strength,
            amount: strength_amount,
          });
          if i != context.monster_index() {
            context.action(GainBlockAction {
              creature_index: CreatureIndex::Monster(i),
              amount: block_amount,
            });
          }
        }
      }
    }
  }
}
