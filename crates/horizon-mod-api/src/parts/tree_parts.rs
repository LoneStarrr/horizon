//! Concrete authoring of tree parts.
use super::*;

impl PartRegistrar {
    /// The id `name` registers under as a `kind`, or why the mods are refused. `taken` says
    /// whether the id is already registered as that kind.
    fn tree_part_id(
        &mut self,
        kind: &str,
        name: &str,
        taken: impl Fn(&TreePartTable, &str) -> bool,
    ) -> Option<Box<str>> {
        let id = match self.owned_declaration_id(name) {
            Ok(id) => id,
            Err(reason) => {
                self.refused.push(reason);
                return None;
            }
        };
        if taken(&self.tree_parts, &id) {
            self.refused
                .push(format!("{} registered {kind} {id} twice", self.mod_id));
            return None;
        }
        Some(id.into())
    }
}

impl TreePartsRegistrar for PartRegistrar {
    fn add_trunk_placer(&mut self, name: &str, ty: Arc<dyn crate::TrunkPlacerType>) {
        let taken = |t: &TreePartTable, id: &str| t.trunk_placers.iter().any(|(i, _)| **i == *id);
        if let Some(id) = self.tree_part_id("trunk placer", name, taken) {
            self.tree_parts.trunk_placers.push((id, ty));
        }
    }
    fn add_foliage_placer(&mut self, name: &str, ty: Arc<dyn crate::FoliagePlacerType>) {
        let taken = |t: &TreePartTable, id: &str| t.foliage_placers.iter().any(|(i, _)| **i == *id);
        if let Some(id) = self.tree_part_id("foliage placer", name, taken) {
            self.tree_parts.foliage_placers.push((id, ty));
        }
    }
    fn add_tree_decorator(&mut self, name: &str, ty: Arc<dyn crate::TreeDecoratorType>) {
        let taken = |t: &TreePartTable, id: &str| t.decorators.iter().any(|(i, _)| **i == *id);
        if let Some(id) = self.tree_part_id("tree decorator", name, taken) {
            self.tree_parts.decorators.push((id, ty));
        }
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
    struct Spiral;
    #[cfg(test)]
    impl mod_api::TrunkPlacerType for Spiral {
        fn decode(&self, _: &serde_json::Value) -> Result<Arc<dyn mod_api::TrunkPlacer>, String> {
            Err("fixture decodes nothing".into())
        }
    }
    #[cfg(test)]
    struct Cross;
    #[cfg(test)]
    impl mod_api::FoliagePlacerType for Cross {
        fn decode(&self, _: &serde_json::Value) -> Result<Arc<dyn mod_api::FoliagePlacer>, String> {
            Err("fixture decodes nothing".into())
        }
    }
    #[cfg(test)]
    struct Lantern;
    #[cfg(test)]
    impl mod_api::TreeDecoratorType for Lantern {
        fn decode(&self, _: &serde_json::Value) -> Result<Arc<dyn mod_api::TreeDecorator>, String> {
            Err("fixture decodes nothing".into())
        }
    }

    /// Every id a mod may not register, per kind, is refused at startup, naming the mod. A trunk
    /// placer and a foliage placer may share a name: they are separate registries.
    #[test]
    fn invalid_tree_parts_refuse_registration() {
        struct Invalid;
        #[mod_api::__subscriptions]
        impl Hooks for Invalid {
            fn register(&self, r: &mut dyn Registrar, _: &mod_api::RegistrationContext) {
                r.register_trunk_placer("spiral", Spiral);
                r.register_foliage_placer("spiral", Cross);
                r.register_trunk_placer("spiral", Spiral);
                r.register_foliage_placer("other:spiral", Cross);
                r.register_tree_decorator("", Lantern);
                r.register_tree_decorator("Bad Name", Lantern);
                r.register_tree_decorator("lantern", Lantern);
                r.register_tree_decorator("lantern", Lantern);
            }
        }
        let errors = install_parts(
            &[ModEntry::new("test", Box::new(Invalid))],
            &crate::parts::tests::rule_schema(),
            |_| false,
        )
        .unwrap_err();
        assert_eq!(errors.len(), 5, "{errors:?}");
        assert!(errors[0].contains("test registered trunk placer test:spiral twice"));
        assert!(errors[1].contains("namespace"), "{errors:?}");
        assert!(
            errors[2].contains("invalid identifier \"test:\""),
            "{errors:?}"
        );
        assert!(
            errors[3].contains("invalid identifier \"test:Bad Name\""),
            "{errors:?}"
        );
        assert!(errors[4].contains("test registered tree decorator test:lantern twice"));
    }

    /// A valid registration is handed over under its namespaced id, per kind.
    #[test]
    fn registered_tree_parts_are_declared_under_their_ids() {
        struct All;
        #[mod_api::__subscriptions]
        impl Hooks for All {
            fn register(&self, r: &mut dyn Registrar, _: &mod_api::RegistrationContext) {
                r.register_trunk_placer("spiral_trunk_placer", Spiral);
                r.register_foliage_placer("cross_foliage_placer", Cross);
                r.register_tree_decorator("lantern", Lantern);
            }
        }
        crate::parts::tests::install_parts_fixture(&[ModEntry::new("mymod", Box::new(All))])
            .unwrap();
        let parts = mod_api::__private::tree_part_declarations();
        assert_eq!(parts.trunk_placers[0].0, "mymod:spiral_trunk_placer");
        assert_eq!(parts.foliage_placers[0].0, "mymod:cross_foliage_placer");
        assert_eq!(parts.decorators[0].0, "mymod:lantern");
    }
}
