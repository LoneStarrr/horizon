use super::*;
/// The density function types registered at startup, for the executable to hand to the engine.
pub(crate) fn installed_density_function_list()
-> &'static [crate::density_function::DensityFunctionInfo] {
    installed().map_or(&[], |installed| installed.density_functions.as_slice())
}

impl dyn Registrar + '_ {
    /// Register density function type `name` (`wave` or `mymod:wave`) once at
    /// startup: a density function in a data pack with `"type": "mymod:wave"`
    /// then samples by `ty`, which decodes its JSON, declares its value range
    /// and builds a sampler from the world's seed. The sampler runs on the
    /// worldgen worker threads. The id is in the calling mod's namespace;
    /// another mod's namespace, the `minecraft` namespace, an empty name, a name outside
    /// those rules and an
    /// id registered twice refuse the mods at startup, naming the mod. The name
    /// follows vanilla's resource location rules: lowercase letters, digits and
    /// `_ . - /`.
    pub fn register_density_function<T: crate::DensityFunctionType>(&mut self, name: &str, ty: T) {
        self.add_density_function(name, Arc::new(ty));
    }
}

/// Registrar operations for density function types.
pub trait DensityRegistrar {
    /// Register density function type `name` with `ty`; mods reach it through
    /// `register_density_function`.
    ///
    /// Not on wasm: density samplers are native code that runs on the worldgen worker threads,
    /// which a wasm mod's cannot hand over.
    fn add_density_function(&mut self, name: &str, ty: Arc<dyn crate::DensityFunctionType>) {
        let _ = ty;
        panic!("density function {name:?}: a wasm mod cannot register a density function type");
    }
}

impl DensityRegistrar for PartRegistrar {
    fn add_density_function(&mut self, name: &str, ty: Arc<dyn crate::DensityFunctionType>) {
        let (namespace, path) = name.split_once(':').unwrap_or((self.mod_id, name));
        let id = format!("{}:{}", self.mod_id, path);
        let refusal = if namespace != self.mod_id {
            Some(format!(
                "{} registered density function {name:?} in the namespace {namespace:?}, which is not its own",
                self.mod_id
            ))
        } else if path.is_empty() {
            Some(format!(
                "{} registered a density function with no name",
                self.mod_id
            ))
        } else if !path
            .bytes()
            .all(|b| matches!(b, b'a'..=b'z' | b'0'..=b'9' | b'_' | b'.' | b'-' | b'/'))
        {
            Some(format!(
                "{} registered density function {id:?}, whose name must be lowercase letters, \
                 digits and _ . - / (a resource location path)",
                self.mod_id
            ))
        } else if self.mod_id == "minecraft" {
            Some(format!(
                "{} registered density function {id}, in the game's own namespace",
                self.mod_id
            ))
        } else {
            self.density_functions
                .iter()
                .any(|d| *d.id == *id)
                .then(|| format!("{} registered density function {id} twice", self.mod_id))
        };
        if let Some(refusal) = refusal {
            self.refused.push(refusal);
            return;
        }
        self.density_functions
            .push(crate::density_function::DensityFunctionInfo {
                id: id.into(),
                ty: Arc::new(crate::density_function::Adapter(ty)),
            });
    }
}

/// Fixtures that tests of other crates use to install Parts.
#[cfg(any(test, feature = "testing"))]
pub mod tests {
    #[cfg(test)]
    use super::*;
    #[cfg(test)]
    use crate::*;

    #[cfg(test)]
    struct Wave;
    #[cfg(test)]
    #[derive(Debug)]
    struct WaveFunction(f32);
    #[cfg(test)]
    #[derive(Debug)]
    struct WaveSampler(f32, i64);
    #[cfg(test)]
    impl mod_api::DensitySampler for WaveSampler {
        fn sample_value(&self, p: [i32; 3]) -> f32 {
            self.0 * p[0] as f32 + self.1 as f32
        }
    }
    #[cfg(test)]
    impl mod_api::CustomDensity for WaveFunction {
        fn range(&self) -> [f32; 2] {
            [-1.0, 1.0]
        }
        fn compile(&self, seed: i64) -> Arc<dyn mod_api::DensitySampler> {
            Arc::new(WaveSampler(self.0, seed))
        }
    }
    #[cfg(test)]
    impl mod_api::DensityFunctionType for Wave {
        fn decode(
            &self,
            config: &serde_json::Value,
        ) -> Result<Arc<dyn mod_api::CustomDensity>, String> {
            let scale = config["scale"].as_f64().ok_or("missing scale")?;
            Ok(Arc::new(WaveFunction(scale as f32)))
        }
    }

    /// Each density function a mod may not register: every one is refused at
    /// startup, naming the mod.
    #[test]
    fn invalid_density_functions_refuse_registration() {
        struct Invalid;
        #[mod_api::__subscriptions]
        impl Hooks for Invalid {
            fn register(&self, r: &mut dyn Registrar, _: &mod_api::RegistrationContext) {
                r.register_density_function("wave", Wave);
                r.register_density_function("wave", Wave);
                r.register_density_function("other:wave", Wave);
                r.register_density_function("", Wave);
                r.register_density_function("Bad Name", Wave);
            }
        }
        let errors = install_parts(
            &[ModEntry::new("test", Box::new(Invalid))],
            &crate::parts::tests::rule_schema(),
        )
        .unwrap_err();
        assert_eq!(errors.len(), 4, "{errors:?}");
        assert!(errors[0].contains("test registered density function test:wave twice"));
        assert!(errors[1].contains("test registered density function \"other:wave\""));
        assert!(errors[2].contains("test registered a density function with no name"));
        assert!(errors[3].contains("test registered density function \"test:Bad Name\""));
    }

    /// The game's own namespace stays the game's.
    #[test]
    fn the_game_namespace_refuses_density_functions() {
        struct Own;
        #[mod_api::__subscriptions]
        impl Hooks for Own {
            fn register(&self, r: &mut dyn Registrar, _: &mod_api::RegistrationContext) {
                r.register_density_function("add", Wave);
            }
        }
        let errors = install_parts(
            &[ModEntry::new("minecraft", Box::new(Own))],
            &crate::parts::tests::rule_schema(),
        )
        .unwrap_err();
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("minecraft registered density function minecraft:add"));
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
        install_parts(
            &[ModEntry::new("test", Box::new(Registers))],
            &crate::parts::tests::rule_schema(),
        )
        .unwrap();
        let types = mod_api::__private::installed_density_functions();
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
}
