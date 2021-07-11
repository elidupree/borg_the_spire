use borg_the_spire::ai_utils::Strategy;
use borg_the_spire::competing_optimizers::playout_result;
use borg_the_spire::seed_system::{SeedView, SingleSeedView, Unseeded};
use borg_the_spire::seeds_concrete::CombatChoiceLineagesKind;
use borg_the_spire::simulation_state::CombatState;
use borg_the_spire::start_and_strategy_ai::{FastStrategy, PurelyRandomStrategy};
use criterion::{criterion_group, criterion_main, Criterion};

fn slimes_benchmark<S: SeedView<CombatState>>(
  id: &str,
  c: &mut Criterion,
  seed: impl Fn() -> S,
  strategy: impl Strategy,
) {
  let slimes_file = std::fs::File::open("data/slimes_benchmark.json").unwrap();
  let slimes_state: CombatState =
    serde_json::from_reader(std::io::BufReader::new(slimes_file)).unwrap();
  c.bench_function(id, |b| {
    b.iter(|| playout_result(&slimes_state, seed(), &strategy))
  });
}

fn slimes_unseeded_random(c: &mut Criterion) {
  slimes_benchmark(
    "slimes_unseeded_random",
    c,
    || Unseeded,
    PurelyRandomStrategy,
  )
}

fn slimes_seeded_random(c: &mut Criterion) {
  slimes_benchmark(
    "slimes_seeded_random",
    c,
    || SingleSeedView::<CombatChoiceLineagesKind>::default(),
    PurelyRandomStrategy,
  )
}

fn slimes_unseeded_faststrategy(c: &mut Criterion) {
  slimes_benchmark(
    "slimes_unseeded_faststrategy",
    c,
    || Unseeded,
    FastStrategy::random(),
  )
}

fn slimes_seeded_faststrategy(c: &mut Criterion) {
  slimes_benchmark(
    "slimes_seeded_faststrategy",
    c,
    || SingleSeedView::<CombatChoiceLineagesKind>::default(),
    FastStrategy::random(),
  )
}

criterion_group!(
  benches,
  slimes_unseeded_random,
  slimes_seeded_random,
  slimes_unseeded_faststrategy,
  slimes_seeded_faststrategy
);
criterion_main!(benches);
