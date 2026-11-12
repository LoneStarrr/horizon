//! Concrete authoring of feature types.
use super::*;

impl FeatureRegistrar for PartRegistrar {
    fn add_feature_type(&mut self, name: &str, ty: Arc<dyn crate::FeatureType>) {
        let id = match self.owned_declaration_id(name) {
            Ok(id) => id,
            Err(reason) => {
                self.refused.push(reason);
                return;
            }
        };
        if self.feature_types.iter().any(|d| *d.id == *id) {
            self.refused.push(format!(
                "{} registered feature type {id} twice",
                self.mod_id
            ));
            return;
        }
        self.feature_types
            .push(FeatureTypeInfo { id: id.into(), ty });
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
    struct Column;
    #[cfg(test)]
    impl mod_api::FeatureType for Column {
        fn decode(
            &self,
            _: &serde_json::Value,
        ) -> Result<Arc<dyn mod_api::ConfiguredFeature>, String> {
            Err("fixture decodes nothing".into())
        }
    }

    /// Each feature type a mod may not register: every one is refused at
    /// startup, naming the mod.
    #[test]
    fn invalid_feature_types_refuse_registration() {
        struct Invalid;
        #[mod_api::__subscriptions]
        impl Hooks for Invalid {
            fn register(&self, r: &mut dyn Registrar, _: &mod_api::RegistrationContext) {
                r.register_feature_type("spikes", Column);
                r.register_feature_type("spikes", Column);
                r.register_feature_type("other:spikes", Column);
                r.register_feature_type("", Column);
                r.register_feature_type("Bad Name", Column);
            }
        }
        let errors = install_parts(
            &[ModEntry::new("test", Box::new(Invalid))],
            &crate::parts::tests::rule_schema(),
            |_| false,
        )
        .unwrap_err();
        assert_eq!(errors.len(), 4, "{errors:?}");
        assert!(errors[0].contains("test registered feature type test:spikes twice"));
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
    fn a_registered_feature_type_is_declared_under_its_id() {
        struct Registers;
        #[mod_api::__subscriptions]
        impl Hooks for Registers {
            fn register(&self, r: &mut dyn Registrar, _: &mod_api::RegistrationContext) {
                r.register_feature_type("column", Column);
            }
        }
        crate::parts::tests::install_parts_fixture(&[ModEntry::new("test", Box::new(Registers))])
            .unwrap();
        let types = mod_api::__private::feature_type_declarations();
        assert_eq!(types.len(), 1);
        assert_eq!(types[0].0, "test:column");
    }
}
