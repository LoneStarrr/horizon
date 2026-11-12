//! Concrete authoring of density function types.
use super::*;

impl DensityRegistrar for PartRegistrar {
    fn add_density_function(&mut self, name: &str, ty: Arc<dyn crate::DensityFunctionType>) {
        let id = match self.owned_declaration_id(name) {
            Ok(id) => id,
            Err(reason) => {
                self.refused.push(reason);
                return;
            }
        };
        if self.density_functions.iter().any(|d| *d.id == *id) {
            self.refused.push(format!(
                "{} registered density function {id} twice",
                self.mod_id
            ));
            return;
        }
        self.density_functions
            .push(DensityFunctionInfo { id: id.into(), ty });
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
    impl mod_api::DensityFunctionType for Wave {
        fn decode(&self, _: &serde_json::Value) -> Result<Arc<dyn mod_api::CustomDensity>, String> {
            Err("fixture decodes nothing".into())
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
            |_| false,
        )
        .unwrap_err();
        assert_eq!(errors.len(), 4, "{errors:?}");
        assert!(errors[0].contains("test registered density function test:wave twice"));
        assert!(errors[1].contains("namespace"), "{errors:?}");
        assert!(
            errors[2].contains("invalid identifier \"test:\""),
            "{errors:?}"
        );
        assert!(
            errors[3].contains("invalid identifier \"test:Bad Name\""),
            "{errors:?}"
        );
    }

    /// A valid registration is handed over under its namespaced id.
    #[test]
    fn a_registered_density_function_is_declared_under_its_id() {
        struct Registers;
        #[mod_api::__subscriptions]
        impl Hooks for Registers {
            fn register(&self, r: &mut dyn Registrar, _: &mod_api::RegistrationContext) {
                r.register_density_function("wave", Wave);
            }
        }
        crate::parts::tests::install_parts_fixture(&[ModEntry::new("test", Box::new(Registers))])
            .unwrap();
        let types = mod_api::__private::density_function_declarations();
        assert_eq!(types.len(), 1);
        assert_eq!(types[0].0, "test:wave");
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
