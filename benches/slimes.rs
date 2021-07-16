use borg_the_spire::ai_utils::playout_result;
use borg_the_spire::ai_utils::Strategy;
use borg_the_spire::seed_system::{Seed, SingleSeedGenerator, TrivialSeedGenerator};
use borg_the_spire::seeds_concrete::CombatChoiceLineagesKind;
use borg_the_spire::simulation_state::CombatState;
use borg_the_spire::start_and_strategy_ai::{FastStrategy, PurelyRandomStrategy};
use criterion::{criterion_group, criterion_main, Criterion};
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;

fn slimes_benchmark<S: Strategy, T: Seed<CombatState>>(
  id: &str,
  c: &mut Criterion,
  mut seed: impl FnMut() -> T,
  mut strategy: impl FnMut() -> S,
  runs_per_seed: usize,
) {
  let slimes_file = std::fs::File::open("data/slimes_benchmark.json").unwrap();
  let slimes_state: CombatState =
    serde_json::from_reader(std::io::BufReader::new(slimes_file)).unwrap();
  c.bench_function(id, |b| {
    b.iter(|| {
      let seed = seed();
      for _ in 0..runs_per_seed {
        playout_result(&slimes_state, seed.view(), &strategy());
      }
    })
  });
}

fn slimes_unseeded_random(c: &mut Criterion) {
  let mut generator = TrivialSeedGenerator::new(ChaCha8Rng::seed_from_u64(0));
  slimes_benchmark(
    "slimes_unseeded_random",
    c,
    || generator.make_seed(),
    || PurelyRandomStrategy,
    1,
  )
}

fn slimes_seeded_random(c: &mut Criterion) {
  let mut generator = SingleSeedGenerator::new(ChaCha8Rng::seed_from_u64(0));
  slimes_benchmark(
    "slimes_seeded_random",
    c,
    || generator.make_seed::<CombatChoiceLineagesKind>(),
    || PurelyRandomStrategy,
    10,
  )
}

fn slimes_unseeded_faststrategy(c: &mut Criterion) {
  let mut seed_generator = TrivialSeedGenerator::new(ChaCha8Rng::seed_from_u64(0));
  let mut strategy_rng = ChaCha8Rng::seed_from_u64(1);
  slimes_benchmark(
    "slimes_unseeded_faststrategy",
    c,
    || seed_generator.make_seed(),
    || FastStrategy::random(&mut strategy_rng),
    1,
  )
}

fn slimes_seeded_faststrategy(c: &mut Criterion) {
  let mut seed_generator = SingleSeedGenerator::new(ChaCha8Rng::seed_from_u64(0));
  let mut strategy_rng = ChaCha8Rng::seed_from_u64(1);
  slimes_benchmark(
    "slimes_seeded_faststrategy",
    c,
    || seed_generator.make_seed::<CombatChoiceLineagesKind>(),
    || FastStrategy::random(&mut strategy_rng),
    10,
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
