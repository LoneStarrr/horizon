use super::*;
use mod_api::{Hooks, ModEntry, Registrar};

struct Wave;
#[derive(Debug)]
struct WaveFunction(f32);
#[derive(Debug)]
struct WaveSampler(f32, i64);
impl mod_api::DensitySampler for WaveSampler {
    fn sample_value(&self, p: [i32; 3]) -> f32 {
        self.0 * p[0] as f32 + self.1 as f32
    }
}
impl mod_api::CustomDensity for WaveFunction {
    fn range(&self) -> [f32; 2] {
        [-1.0, 1.0]
    }
    fn compile(&self, seed: i64) -> Arc<dyn mod_api::DensitySampler> {
        Arc::new(WaveSampler(self.0, seed))
    }
}
impl mod_api::DensityFunctionType for Wave {
    fn decode(
        &self,
        config: &serde_json::Value,
    ) -> Result<Arc<dyn mod_api::CustomDensity>, String> {
        let scale = config["scale"].as_f64().ok_or("missing scale")?;
        Ok(Arc::new(WaveFunction(scale as f32)))
    }
}

/// What the engine decodes is the mod's own code, under its namespaced id.
#[test]
fn a_registered_density_function_reaches_the_engine() {
    struct Registers;
    #[mod_api::__subscriptions]
    impl Hooks for Registers {
        fn register(&self, r: &mut dyn Registrar, _: &mod_api::RegistrationContext) {
            r.register_density_function("wave", Wave);
        }
    }
    mod_platform::__private::part_tests::install_parts_fixture(&[ModEntry::new(
        "test",
        Box::new(Registers),
    )])
    .unwrap();
    let types = installed_density_functions();
    let function = types["test:wave"]
        .decode(&serde_json::json!({"type": "test:wave", "scale": 0.5}))
        .unwrap();
    assert_eq!(function.range(), [-1.0, 1.0]);
    let engine_sampler = function.compile(100);
    let mut context = horizon_core::density::SamplerContext::default();
    assert_eq!(
        horizon_core::density::DensitySampler::sample_value(
            &*engine_sampler,
            &mut context,
            [6, 0, 0]
        ),
        Ok(103.0)
    );
    assert!(types["test:wave"].decode(&serde_json::json!({})).is_err());
}

/// A mod builds its seeded randoms and noise from the exported types
/// alone; the first draw is vanilla's Xoroshiro128++ for seed 0.
#[test]
fn exported_random_and_noise_types_build_a_sampler_input() {
    use mod_api::{NormalNoise, XoroshiroRandom};
    assert_eq!(XoroshiroRandom::new(0).next_long(), 3038984756725240190);
    let mut random = XoroshiroRandom::new(7);
    let positional = random.fork_positional();
    let drift = NormalNoise::create_parity(-10, &[1.0, 0.5])
        .unwrap()
        .create(&mut positional.from_hash_of("mymod:drift"))
        .unwrap();
    let [min, max] = drift.range().unwrap();
    let value = drift.get3(12.5, 0.0, -3.25);
    assert!(min <= value && value <= max);
    let again = NormalNoise::create_parity(-10, &[1.0, 0.5])
        .unwrap()
        .create(
            &mut XoroshiroRandom::new(7)
                .fork_positional()
                .from_hash_of("mymod:drift"),
        )
        .unwrap();
    assert_eq!(again.get3(12.5, 0.0, -3.25), value);
}
