use super::*;

intent! {
  pub enum CultistIntent {
    3: Incantation,
    1: DarkStrike,
  }
}
impl MonsterBehavior for Cultist {
  type Intent = CultistIntent;
  fn make_intent_distribution(context: &mut IntentChoiceContext) {
    use CultistIntent::*;
    context.always(if context.first_move() {
      Incantation
    } else {
      DarkStrike
    });
  }
  fn intent_effects(context: &mut impl IntentEffectsContext) {
    use CultistIntent::*;
    match context.intent::<Self::Intent>() {
      DarkStrike => context.attack(6),
      Incantation => context.power_self(
        PowerId::Ritual,
        context.with_ascensions(Ascension(17), 5, Ascension(2), 4, 3),
      ),
    }
  }
}

intent! {
  pub enum RedLouseIntent {
    3: Bite,
    4: Grow,
  }
}
impl MonsterBehavior for RedLouse {
  type Intent = RedLouseIntent;
  fn make_intent_distribution(context: &mut IntentChoiceContext) {
    use RedLouseIntent::*;
    let max_buff_repeats = Repeats(context.with_ascension(Ascension(17), 1, 2));
    context.if_num_lt(25, context.with_max_repeats(max_buff_repeats, Grow, Bite));
    context.else_num(context.with_max_repeats(Repeats(2), Bite, Grow));
  }
  fn after_choosing_intent(runner: &mut impl Runner, monster_index: usize) {
    if runner.state().monster_intent(monster_index) == 3 {
      let ascension = runner.state().monsters[monster_index].ascension;
      let bonus = if ascension >= 2 { 1 } else { 0 };
      runner.action_now(&InitializeMonsterInnateDamageAmount {
        monster_index,
        range: (5 + bonus, 8 + bonus),
      });
    }
  }
  fn intent_effects(context: &mut impl IntentEffectsContext) {
    use RedLouseIntent::*;
    match context.intent::<Self::Intent>() {
      Bite => context.attack(context.monster().innate_damage_amount.unwrap()),
      Grow => context.power_self(
        PowerId::Strength,
        context.with_ascension(Ascension(17), 4, 3),
      ),
    }
  }
}

intent! {
  pub enum GreenLouseIntent {
    3: Bite,
    4: SpitWeb,
  }
}
impl MonsterBehavior for GreenLouse {
  type Intent = GreenLouseIntent;
  fn make_intent_distribution(context: &mut IntentChoiceContext) {
    use GreenLouseIntent::*;
    let max_buff_repeats = Repeats(context.with_ascension(Ascension(17), 1, 2));
    context.if_num_lt(
      25,
      context.with_max_repeats(max_buff_repeats, SpitWeb, Bite),
    );
    context.else_num(context.with_max_repeats(Repeats(2), Bite, SpitWeb));
  }
  fn after_choosing_intent(runner: &mut impl Runner, monster_index: usize) {
    RedLouse::after_choosing_intent(runner, monster_index);
  }

  fn intent_effects(context: &mut impl IntentEffectsContext) {
    use GreenLouseIntent::*;
    match context.intent::<Self::Intent>() {
      GreenLouseIntent::Bite => context.attack(context.monster().innate_damage_amount.unwrap()),
      SpitWeb => context.power_player(PowerId::Weak, 2),
    }
  }
}

intent! {
  pub enum JawWormIntent {
    1: Chomp,
    3: Thrash,
    2: Bellow,
  }
}
impl MonsterBehavior for JawWorm {
  type Intent = JawWormIntent;
  fn make_intent_distribution(context: &mut IntentChoiceContext) {
    use JawWormIntent::*;
    if context.first_move() {
      context.always(1);
    }
    context.if_num_lt(
      25,
      context.with_max_repeats(Repeats(1), Chomp, split(0.5625, Bellow, Thrash)),
    );
    context.if_num_lt(
      55,
      context.with_max_repeats(Repeats(2), Thrash, split(0.357, Chomp, 2)),
    );
    context.else_num(context.with_max_repeats(Repeats(1), Bellow, split(0.416, Chomp, Thrash)));
  }
  fn intent_effects(context: &mut impl IntentEffectsContext) {
    use JawWormIntent::*;
    match context.intent::<Self::Intent>() {
      Chomp => context.attack(context.with_ascension(Ascension(2), 12, 11)),
      Bellow => {
        context.power_self(
          PowerId::Strength,
          context.with_ascensions(Ascension(17), 5, Ascension(2), 4, 3),
        );
        context.block(context.with_ascension(Ascension(17), 9, 6));
      }
      Thrash => {
        context.attack(7);
        context.block(5);
      }
    }
  }
}

intent! {
  pub enum AcidSlimeSIntent {
    2: Lick,
    1: Tackle,
  }
}
impl MonsterBehavior for AcidSlimeS {
  type Intent = AcidSlimeSIntent;
  fn make_intent_distribution(context: &mut IntentChoiceContext) {
    use AcidSlimeSIntent::*;
    if let Some(last_intent) = context.last_intent::<Self::Intent>() {
      context.always(match last_intent {
        Lick => Tackle,
        Tackle => Lick,
      });
    } else if context.ascension() >= 17 {
      context.always(Lick);
    } else {
      context.always(split(0.5, Tackle, Lick));
    }
  }
  fn intent_effects(context: &mut impl IntentEffectsContext) {
    use AcidSlimeSIntent::*;
    match context.intent::<Self::Intent>() {
      Tackle => context.attack(context.with_ascension(Ascension(2), 4, 3)),
      Lick => context.power_player(PowerId::Weak, 1),
    }
  }
}

intent! {
  pub enum AcidSlimeMIntent {
    2: Tackle,
    1: CorrosiveSpit,
    4: Lick,
  }
}
impl MonsterBehavior for AcidSlimeM {
  type Intent = AcidSlimeMIntent;
  fn make_intent_distribution(context: &mut IntentChoiceContext) {
    use AcidSlimeMIntent::*;
    if context.ascension() >= 17 {
      context.if_num_lt(
        40,
        context.with_max_repeats(Repeats(2), CorrosiveSpit, split(0.5, Tackle, Lick)),
      );
      context.if_num_lt(
        80,
        context.with_max_repeats(Repeats(2), Tackle, split(0.5, CorrosiveSpit, Lick)),
      );
      context.else_num(context.with_max_repeats(
        Repeats(1),
        Lick,
        split(0.4, CorrosiveSpit, Tackle),
      ));
    } else {
      context.if_num_lt(
        30,
        context.with_max_repeats(Repeats(2), CorrosiveSpit, split(0.5, Tackle, Lick)),
      );
      context.if_num_lt(
        70,
        context.with_max_repeats(Repeats(1), Tackle, split(0.4, CorrosiveSpit, Lick)),
      );
      context.else_num(context.with_max_repeats(
        Repeats(2),
        Lick,
        split(0.4, CorrosiveSpit, Tackle),
      ));
    }
  }
  fn intent_effects(context: &mut impl IntentEffectsContext) {
    use AcidSlimeMIntent::*;
    match context.intent::<Self::Intent>() {
      CorrosiveSpit => {
        context.attack(context.with_ascension(Ascension(2), 8, 7));
        context.discard_status(CardId::Slimed, 1);
      }
      Tackle => context.attack(context.with_ascension(Ascension(2), 12, 10)),
      Lick => context.power_player(PowerId::Weak, 1),
    }
  }
}

intent! {
  pub enum AcidSlimeLIntent {
    2: Tackle,
    1: CorrosiveSpit,
    4: Lick,
    3: Split,
  }
}
impl MonsterBehavior for AcidSlimeL {
  type Intent = AcidSlimeLIntent;
  fn make_intent_distribution(context: &mut IntentChoiceContext) {
    use AcidSlimeLIntent::*;
    if context.ascension() >= 17 {
      context.if_num_lt(
        40,
        context.with_max_repeats(Repeats(2), CorrosiveSpit, split(0.6, Tackle, Lick)),
      );
      context.if_num_lt(
        70,
        context.with_max_repeats(Repeats(2), Tackle, split(0.6, CorrosiveSpit, Lick)),
      );
      context.else_num(context.with_max_repeats(
        Repeats(1),
        Lick,
        split(0.4, CorrosiveSpit, Tackle),
      ));
    } else {
      context.if_num_lt(
        30,
        context.with_max_repeats(Repeats(2), CorrosiveSpit, split(0.5, Tackle, Lick)),
      );
      context.if_num_lt(
        70,
        context.with_max_repeats(Repeats(1), Tackle, split(0.4, CorrosiveSpit, Lick)),
      );
      context.else_num(context.with_max_repeats(
        Repeats(2),
        Lick,
        split(0.4, CorrosiveSpit, Tackle),
      ));
    }
  }
  fn intent_effects(context: &mut impl IntentEffectsContext) {
    use AcidSlimeLIntent::*;
    if context.monster().creature.hitpoints * 2 <= context.monster().creature.max_hitpoints {
      context.action(SplitAction(
        context.monster_index(),
        [MonsterId::AcidSlimeM, MonsterId::AcidSlimeM],
      ));
      return;
    }
    match context.intent::<Self::Intent>() {
      CorrosiveSpit => {
        context.attack(context.with_ascension(Ascension(2), 12, 11));
        context.discard_status(CardId::Slimed, 2);
      }
      Tackle => context.attack(context.with_ascension(Ascension(2), 18, 16)),
      Lick => context.power_player(PowerId::Weak, 2),
      Split => unreachable!(),
    }
  }
}

intent! {
  pub enum SpikeSlimeSIntent {
    1: Tackle,
  }
}
impl MonsterBehavior for SpikeSlimeS {
  type Intent = SpikeSlimeSIntent;
  fn make_intent_distribution(context: &mut IntentChoiceContext) {
    use SpikeSlimeSIntent::*;
    context.always(Tackle);
  }
  fn intent_effects(context: &mut impl IntentEffectsContext) {
    use SpikeSlimeSIntent::*;
    match context.intent::<Self::Intent>() {
      Tackle => context.attack(context.with_ascension(Ascension(2), 6, 5)),
    }
  }
}

intent! {
  pub enum SpikeSlimeMIntent {
    1: FlameTackle,
    4: Lick,
  }
}
impl MonsterBehavior for SpikeSlimeM {
  type Intent = SpikeSlimeMIntent;
  fn make_intent_distribution(context: &mut IntentChoiceContext) {
    use SpikeSlimeMIntent::*;
    let max_debuff_repeats = Repeats(context.with_ascension(Ascension(17), 1, 2));
    context.if_num_lt(30, context.with_max_repeats(Repeats(2), FlameTackle, Lick));
    context.else_num(context.with_max_repeats(max_debuff_repeats, Lick, FlameTackle));
  }
  fn intent_effects(context: &mut impl IntentEffectsContext) {
    use SpikeSlimeMIntent::*;
    match context.intent::<Self::Intent>() {
      FlameTackle => {
        context.attack(context.with_ascension(Ascension(2), 10, 8));
        context.discard_status(CardId::Slimed, 1);
      }
      Lick => context.power_player(PowerId::Frail, 1),
    }
  }
}

intent! {
  pub enum SpikeSlimeLIntent {
    1: FlameTackle,
    4: Lick,
    3: Split,
  }
}
impl MonsterBehavior for SpikeSlimeL {
  type Intent = SpikeSlimeLIntent;
  fn make_intent_distribution(context: &mut IntentChoiceContext) {
    use SpikeSlimeLIntent::*;
    let max_debuff_repeats = Repeats(context.with_ascension(Ascension(17), 1, 2));
    context.if_num_lt(30, context.with_max_repeats(Repeats(2), FlameTackle, Lick));
    context.else_num(context.with_max_repeats(max_debuff_repeats, Lick, FlameTackle));
  }
  fn intent_effects(context: &mut impl IntentEffectsContext) {
    use SpikeSlimeLIntent::*;
    if context.monster().creature.hitpoints * 2 <= context.monster().creature.max_hitpoints {
      context.action(SplitAction(
        context.monster_index(),
        [MonsterId::SpikeSlimeM, MonsterId::SpikeSlimeM],
      ));
      return;
    }
    match context.intent::<Self::Intent>() {
      FlameTackle => {
        context.attack(context.with_ascension(Ascension(2), 18, 16));
        context.discard_status(CardId::Slimed, 1);
      }
      Lick => context.power_player(PowerId::Frail, 2),
      Split => unreachable!(),
    }
  }
}

intent! {
  pub enum FungiBeastIntent {
    1: Bite,
    2: Grow,
  }
}
impl MonsterBehavior for FungiBeast {
  type Intent = FungiBeastIntent;
  fn make_intent_distribution(context: &mut IntentChoiceContext) {
    use FungiBeastIntent::*;
    context.if_num_lt(60, context.with_max_repeats(Repeats(2), Bite, Grow));
    context.else_num(context.with_max_repeats(Repeats(1), Grow, Bite));
  }
  fn intent_effects(context: &mut impl IntentEffectsContext) {
    use FungiBeastIntent::*;
    match context.intent::<Self::Intent>() {
      Bite => context.attack(6),
      Grow => context.power_self(
        PowerId::Strength,
        context.with_ascension(Ascension(17), 4, 3),
      ),
    }
  }
}
intent! {
  pub enum LooterIntent {
    1: Mug,
    4: Lunge,
    2: SmokeBomb,
    3: Escape,
  }
}
impl MonsterBehavior for Looter {
  type Intent = LooterIntent;
  fn make_intent_distribution(context: &mut IntentChoiceContext) {
    use LooterIntent::*;
    if context.state().turn_number < 2 {
      context.always(Mug);
    } else if context.state().turn_number == 2 {
      context.always(split(0.5, Lunge, SmokeBomb));
    } else {
      context.always(context.with_max_repeats(Repeats(1), SmokeBomb, Escape));
    }
  }
  fn intent_effects(context: &mut impl IntentEffectsContext) {
    use LooterIntent::*;
    match context.intent::<Self::Intent>() {
      Mug => context.attack(context.with_ascension(Ascension(2), 11, 10)),
      Lunge => context.attack(context.with_ascension(Ascension(2), 14, 12)),
      SmokeBomb => context.block(6),
      Escape => context.action(EscapeAction(context.monster_index())),
    }
  }
}
intent! {
  pub enum SlaverBlueIntent {
    1: Stab,
    4: Rake,
  }
}
impl MonsterBehavior for SlaverBlue {
  type Intent = SlaverBlueIntent;
  fn make_intent_distribution(context: &mut IntentChoiceContext) {
    use SlaverBlueIntent::*;
    if !context.did_repeats(Repeats(2), Stab) {
      context.if_num_geq(40, Stab);
    }

    let max_rake_repeats = Repeats(context.with_ascension(Ascension(17), 1, 2));
    context.else_num(context.with_max_repeats(max_rake_repeats, Rake, Stab));
  }
  fn intent_effects(context: &mut impl IntentEffectsContext) {
    use SlaverBlueIntent::*;
    match context.intent::<Self::Intent>() {
      Stab => context.attack(context.with_ascension(Ascension(2), 13, 12)),
      Rake => {
        context.attack(context.with_ascension(Ascension(2), 8, 7));
        context.power_player(PowerId::Weak, context.with_ascension(Ascension(17), 2, 1));
      }
    }
  }
}
intent! {
  pub enum SlaverRedIntent {
    1: Stab,
    3: Scrape,
    2: Entangle,
  }
}
impl MonsterBehavior for SlaverRed {
  type Intent = SlaverRedIntent;
  fn make_intent_distribution(context: &mut IntentChoiceContext) {
    use SlaverRedIntent::*;
    if context.first_move() {
      context.always(Stab);
      return;
    }

    if context
      .monster()
      .move_history
      .iter()
      .all(|&intent| intent != Entangle.id())
    {
      context.if_num_geq(75, Entangle);
    } else if !context.did_repeats(Repeats(2), Stab) {
      context.if_num_geq(55, Stab);
    }

    let max_scrape_repeats = Repeats(context.with_ascension(Ascension(17), 1, 2));
    context.else_num(context.with_max_repeats(max_scrape_repeats, Scrape, Stab));
  }
  fn intent_effects(context: &mut impl IntentEffectsContext) {
    use SlaverRedIntent::*;
    match context.intent::<Self::Intent>() {
      Stab => context.attack(context.with_ascension(Ascension(2), 14, 13)),
      Entangle => context.power_player(PowerId::Entangled, 1),
      Scrape => {
        context.attack(context.with_ascension(Ascension(2), 9, 8));
        context.power_player(
          PowerId::Vulnerable,
          context.with_ascension(Ascension(17), 2, 1),
        );
      }
    }
  }
}
intent! {
  pub enum MadGremlinIntent {
    1: Scratch,
  }
}
impl MonsterBehavior for MadGremlin {
  type Intent = MadGremlinIntent;
  fn make_intent_distribution(context: &mut IntentChoiceContext) {
    use MadGremlinIntent::*;
    context.always(Scratch);
  }
  fn intent_effects(context: &mut impl IntentEffectsContext) {
    use MadGremlinIntent::*;
    match context.intent::<Self::Intent>() {
      Scratch => context.attack(context.with_ascension(Ascension(2), 5, 4)),
    }
  }
}
intent! {
  pub enum SneakyGremlinIntent {
    1: Puncture,
  }
}
impl MonsterBehavior for SneakyGremlin {
  type Intent = SneakyGremlinIntent;
  fn make_intent_distribution(context: &mut IntentChoiceContext) {
    use SneakyGremlinIntent::*;
    context.always(Puncture);
  }
  fn intent_effects(context: &mut impl IntentEffectsContext) {
    use SneakyGremlinIntent::*;
    match context.intent::<Self::Intent>() {
      Puncture => context.attack(context.with_ascension(Ascension(2), 10, 9)),
    }
  }
}
intent! {
  pub enum GremlinWizardIntent {
    2: Charging,
    1: UltimateBlast,
  }
}
impl MonsterBehavior for GremlinWizard {
  type Intent = GremlinWizardIntent;
  fn make_intent_distribution(context: &mut IntentChoiceContext) {
    use GremlinWizardIntent::*;
    if context.state().turn_number >= 3
      && (context.ascension() >= 17 || (context.state().turn_number % 4) == 3)
    {
      context.always(UltimateBlast);
    } else {
      context.always(Charging);
    }
  }
  fn intent_effects(context: &mut impl IntentEffectsContext) {
    use GremlinWizardIntent::*;
    match context.intent::<Self::Intent>() {
      UltimateBlast => context.attack(context.with_ascension(Ascension(2), 30, 25)),
      Charging => (),
    }
  }
}
intent! {
  pub enum FatGremlinIntent {
    2: Smash,
  }
}
impl MonsterBehavior for FatGremlin {
  type Intent = FatGremlinIntent;
  fn make_intent_distribution(context: &mut IntentChoiceContext) {
    use FatGremlinIntent::*;
    context.always(Smash);
  }
  fn intent_effects(context: &mut impl IntentEffectsContext) {
    use FatGremlinIntent::*;
    match context.intent::<Self::Intent>() {
      Smash => {
        context.attack(context.with_ascension(Ascension(2), 5, 4));
        context.power_player(PowerId::Weak, 1);
        if context.ascension() >= 17 {
          context.power_player(PowerId::Frail, 1);
        }
      }
    }
  }
}
intent! {
  pub enum ShieldGremlinIntent {
    1: Protect,
    2: ShieldBash,
  }
}
impl MonsterBehavior for ShieldGremlin {
  type Intent = ShieldGremlinIntent;
  fn make_intent_distribution(context: &mut IntentChoiceContext) {
    use ShieldGremlinIntent::*;
    if context
      .state()
      .monsters
      .iter()
      .filter(|monster| !monster.gone)
      .count()
      > 1
    {
      context.always(Protect);
    } else {
      context.always(ShieldBash);
    }
  }
  fn intent_effects(context: &mut impl IntentEffectsContext) {
    use ShieldGremlinIntent::*;
    match context.intent::<Self::Intent>() {
      Protect => {
        let amount = context.with_ascensions(Ascension(17), 11, Ascension(7), 8, 7);
        context.action(GainBlockRandomMonsterAction {
          source: context.monster_index(),
          amount,
        });
      }
      ShieldBash => context.attack(context.with_ascension(Ascension(2), 8, 6)),
    }
  }
}
intent! {
  pub enum SentryIntent {
    4: Beam,
    3: Bolt,
  }
}
impl MonsterBehavior for Sentry {
  type Intent = SentryIntent;
  fn make_intent_distribution(context: &mut IntentChoiceContext) {
    use SentryIntent::*;
    if let Some(last_intent) = context.last_intent::<Self::Intent>() {
      context.always(match last_intent {
        Beam => Bolt,
        Bolt => Beam,
      });
    } else {
      if context.monster_index() % 2 == 0 {
        context.always(Bolt);
      } else {
        context.always(Beam);
      }
    }
  }
  fn intent_effects(context: &mut impl IntentEffectsContext) {
    use SentryIntent::*;
    match context.intent::<Self::Intent>() {
      Bolt => context.discard_status(CardId::Dazed, context.with_ascension(Ascension(18), 3, 2)),
      Beam => context.attack(context.with_ascension(Ascension(3), 10, 9)),
    }
  }
}
intent! {
  pub enum GremlinNobIntent {
    3: Bellow,
    1: Rush,
    2: SkullBash,
  }
}
impl MonsterBehavior for GremlinNob {
  type Intent = GremlinNobIntent;
  fn make_intent_distribution(context: &mut IntentChoiceContext) {
    use GremlinNobIntent::*;
    if context.first_move() {
      context.always(Bellow);
      return;
    }

    if context.ascension() >= 18 {
      if (context.state().turn_number % 3) == 2 {
        context.always(SkullBash);
      } else {
        context.always(Rush);
      }
    } else {
      context.if_num_lt(33, SkullBash);
      context.else_num(context.with_max_repeats(Repeats(2), Rush, SkullBash));
    }
  }
  fn intent_effects(context: &mut impl IntentEffectsContext) {
    use GremlinNobIntent::*;
    match context.intent::<Self::Intent>() {
      Rush => context.attack(context.with_ascension(Ascension(3), 16, 14)),
      SkullBash => {
        context.attack(context.with_ascension(Ascension(3), 8, 6));
        context.power_player(PowerId::Vulnerable, 2);
      }
      Bellow => context.power_self(PowerId::Enrage, context.with_ascension(Ascension(18), 3, 2)),
    }
  }
}
intent! {
  pub enum LagavulinIntent {
    5: Sleep,
    3: Attack,
    1: SiphonSoul,
  }
}
impl MonsterBehavior for Lagavulin {
  type Intent = LagavulinIntent;
  fn make_intent_distribution(context: &mut IntentChoiceContext) {
    use LagavulinIntent::*;
    if context.state().turn_number >= 3
      || context.monster().creature.hitpoints < context.monster().creature.max_hitpoints
      || context
        .monster()
        .move_history
        .iter()
        .any(|&intent| intent == 1 || intent == 3)
    {
      context.always(context.with_max_repeats(Repeats(2), Attack, SiphonSoul));
    } else {
      context.always(Sleep);
    }
  }
  fn after_choosing_intent(runner: &mut impl Runner, monster_index: usize) {
    use LagavulinIntent::*;
    let monster = &runner.state().monsters[monster_index];
    let intent = Self::Intent::from_id(monster.intent());
    if intent == SiphonSoul
      || intent == Attack && monster.creature.power_amount(PowerId::Metallicize) >= 8
    {
      runner.action_bottom(ReducePowerAction {
        target: CreatureIndex::Monster(monster_index),
        power_id: PowerId::Metallicize,
        amount: 8,
      });
    }
  }
  fn intent_effects(context: &mut impl IntentEffectsContext) {
    use LagavulinIntent::*;
    match context.intent::<Self::Intent>() {
      SiphonSoul => {
        let amount = context.with_ascension(Ascension(18), -2, -1);
        context.power_player(PowerId::Dexterity, amount);
        context.power_player(PowerId::Strength, amount);
      }
      Attack => context.attack(context.with_ascension(Ascension(3), 20, 18)),
      Sleep => {}
    }
  }
}

intent! {
  pub enum TheGuardianIntent {
    6: ChargingUp,
    2: FierceBash,
    7: VentSteam,
    5: Whirlwind,
    1: DefensiveMode,
    3: RollAttack,
    4: TwinSlam,
  }
}
impl MonsterBehavior for TheGuardian {
  type Intent = TheGuardianIntent;
  fn make_intent_distribution(context: &mut IntentChoiceContext) {
    use TheGuardianIntent::*;
    // TODO
    context.always(Whirlwind);
  }
  fn intent_effects(context: &mut impl IntentEffectsContext) {
    use TheGuardianIntent::*;
    if context.monster().creature.hitpoints * 2 <= context.monster().creature.max_hitpoints {
      context.action(SplitAction(
        context.monster_index(),
        [MonsterId::SpikeSlimeL, MonsterId::AcidSlimeL],
      ));
      return;
    }
    match context.intent::<Self::Intent>() {
      ChargingUp => context.block(9),
      FierceBash => context.attack(context.with_ascension(Ascension(4), 36, 32)),
      VentSteam => {
        context.power_player(PowerId::Weak, 2);
        context.power_player(PowerId::Vulnerable, 2);
      }
      Whirlwind => {
        for _ in 0..4 {
          context.attack(5)
        }
      }
      TwinSlam => {
        // TODO change mode
        for _ in 0..2 {
          context.attack(context.with_ascension(Ascension(4), 36, 32))
        }
        context.action(RemoveSpecificPowerAction {
          target: context.creature_index(),
          power_id: PowerId::SharpHide,
        })
      }
      DefensiveMode => context.power_self(
        PowerId::SharpHide,
        context.with_ascension(Ascension(19), 4, 3),
      ),
      RollAttack => context.attack(context.with_ascension(Ascension(4), 10, 9)),
    }
  }
}

intent! {
  pub enum HexaghostIntent {
    5: Activate,
    1: Divider,
    6: Inferno,
    4: Sear,
    2: Tackle,
    3: Inflame,
  }
}
impl MonsterBehavior for Hexaghost {
  type Intent = HexaghostIntent;
  fn make_intent_distribution(context: &mut IntentChoiceContext) {
    use HexaghostIntent::*;
    let turn = context.state().turn_number;
    if turn == 0 {
      context.always(Activate);
    } else if turn == 1 {
      context.always(Divider);
    } else {
      context.always(match (turn - 2) % 7 {
        0 | 2 | 5 => Sear,
        1 | 4 => Tackle,
        3 => Inflame,
        6 => Inferno,
        _ => unreachable!(),
      });
    }
  }
  fn intent_effects(context: &mut impl IntentEffectsContext) {
    use HexaghostIntent::*;
    match context.intent::<Self::Intent>() {
      Activate => {
        let amount = context.state().player.creature.hitpoints / 12 + 1;
        context.action(InitializeMonsterInnateDamageAmount {
          monster_index: context.monster_index(),
          range: (amount, amount + 1),
        });
      }
      Divider => {
        for _ in 0..6 {
          context.attack(context.monster().innate_damage_amount.unwrap());
        }
      }
      Tackle => {
        for _ in 0..2 {
          context.attack(context.with_ascension(Ascension(4), 6, 5));
        }
      }
      Sear => {
        context.attack(6);
        let upgraded = context.state().turn_number >= 8;
        // TODO: apply upgrade
        context.discard_status(CardId::Burn, context.with_ascension(Ascension(19), 2, 1));
      }
      Inflame => {
        context.power_self(
          PowerId::Strength,
          context.with_ascension(Ascension(19), 3, 2),
        );
        context.block(12);
      }
      Inferno => {
        for _ in 0..6 {
          context.attack(context.with_ascension(Ascension(4), 3, 2));
        }
        for _ in 0..3 {
          context.discard_status(CardId::Burn, 3);
          // TODO: upgrade all burns
        }
      }
    }
  }
}

intent! {
  pub enum SlimeBossIntent {
    4: GoopSpray,
    2: Preparing,
    1: Slam,
    3: Split,
  }
}
impl MonsterBehavior for SlimeBoss {
  type Intent = SlimeBossIntent;
  fn make_intent_distribution(context: &mut IntentChoiceContext) {
    use SlimeBossIntent::*;
    context.always(match context.state().turn_number % 3 {
      0 => GoopSpray,
      1 => Preparing,
      2 => Slam,
      _ => unreachable!(),
    });
  }
  fn intent_effects(context: &mut impl IntentEffectsContext) {
    use SlimeBossIntent::*;
    if context.monster().creature.hitpoints * 2 <= context.monster().creature.max_hitpoints {
      context.action(SplitAction(
        context.monster_index(),
        [MonsterId::SpikeSlimeL, MonsterId::AcidSlimeL],
      ));
      return;
    }
    match context.intent::<Self::Intent>() {
      GoopSpray => {
        context.discard_status(CardId::Slimed, context.with_ascension(Ascension(19), 5, 3))
      }
      Preparing => {}
      Slam => context.attack(context.with_ascension(Ascension(4), 38, 35)),
      Split => {}
    }
  }
}
