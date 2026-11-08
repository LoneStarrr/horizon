use super::*;
/// The feature types registered at startup, for the executable to hand to the engine.
pub(crate) fn installed_feature_type_list() -> &'static [crate::feature_type::FeatureTypeInfo] {
    installed().map_or(&[], |installed| installed.feature_types.as_slice())
}

impl dyn Registrar + '_ {
    /// Register feature type `name` (`spikes` or `mymod:spikes`) once at
    /// startup: a configured feature in a data pack with `"type": "mymod:spikes"`
    /// then places by `ty`, which decodes its JSON and places blocks through a
    /// [`FeatureRegion`](crate::FeatureRegion) with the decorator's random. The
    /// feature runs on the worldgen worker threads. The id is in the calling
    /// mod's namespace; another mod's namespace, the `minecraft` namespace, an
    /// empty name, a name outside vanilla's resource location rules (lowercase
    /// letters, digits and `_ . - /`) and an id registered twice refuse the
    /// mods at startup, naming the mod.
    pub fn register_feature_type<T: crate::FeatureType>(&mut self, name: &str, ty: T) {
        self.add_feature_type(name, Arc::new(ty));
    }
}

/// Registrar operations for feature types.
pub trait FeatureRegistrar {
    /// Register feature type `name` with `ty`; mods reach it through
    /// `register_feature_type`.
    ///
    /// Not on wasm: features are native code that runs on the worldgen worker threads, which a
    /// wasm mod's cannot hand over.
    fn add_feature_type(&mut self, name: &str, ty: Arc<dyn crate::FeatureType>) {
        let _ = ty;
        panic!("feature type {name:?}: a wasm mod cannot register a feature type");
    }
}

impl FeatureRegistrar for PartRegistrar {
    fn add_feature_type(&mut self, name: &str, ty: Arc<dyn crate::FeatureType>) {
        let (namespace, path) = name.split_once(':').unwrap_or((self.mod_id, name));
        let id = format!("{}:{}", self.mod_id, path);
        let refusal = if namespace != self.mod_id {
            Some(format!(
                "{} registered feature type {name:?} in the namespace {namespace:?}, which is not its own",
                self.mod_id
            ))
        } else if path.is_empty() {
            Some(format!(
                "{} registered a feature type with no name",
                self.mod_id
            ))
        } else if !path
            .bytes()
            .all(|b| matches!(b, b'a'..=b'z' | b'0'..=b'9' | b'_' | b'.' | b'-' | b'/'))
        {
            Some(format!(
                "{} registered feature type {id:?}, whose name must be lowercase letters, \
                 digits and _ . - / (a resource location path)",
                self.mod_id
            ))
        } else if self.mod_id == "minecraft" {
            Some(format!(
                "{} registered feature type {id}, in the game's own namespace",
                self.mod_id
            ))
        } else {
            self.feature_types
                .iter()
                .any(|d| *d.id == *id)
                .then(|| format!("{} registered feature type {id} twice", self.mod_id))
        };
        if let Some(refusal) = refusal {
            self.refused.push(refusal);
            return;
        }
        self.feature_types
            .push(crate::feature_type::FeatureTypeInfo {
                id: id.into(),
                ty: Arc::new(crate::feature_type::Adapter(ty)),
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

    /// A column of `height` gold blocks at an x the random picks.
    #[cfg(test)]
    struct Column;
    #[cfg(test)]
    #[derive(Debug)]
    struct ColumnFeature(i32);
    #[cfg(test)]
    impl mod_api::ConfiguredFeature for ColumnFeature {
        fn place(
            &self,
            region: &mut dyn mod_api::FeatureRegion,
            random: &mut dyn mod_api::FeatureRandom,
            origin: [i32; 3],
        ) -> bool {
            let Some(gold) = region.state("gold_block", &[]) else {
                return false;
            };
            let x = origin[0] + random.next_int_bound(4);
            (0..self.0).fold(false, |placed, dy| {
                let at = [x, origin[1] + dy, origin[2]];
                let air = region.block(at).is_some_and(|s| region.is_air(s));
                (air && region.set_block(at, gold, 2)) || placed
            })
        }
    }
    #[cfg(test)]
    impl mod_api::FeatureType for Column {
        fn decode(
            &self,
            config: &serde_json::Value,
        ) -> Result<Arc<dyn mod_api::ConfiguredFeature>, String> {
            let height = config["height"].as_i64().ok_or("missing height")?;
            Ok(Arc::new(ColumnFeature(height as i32)))
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
        )
        .unwrap_err();
        assert_eq!(errors.len(), 4, "{errors:?}");
        assert!(errors[0].contains("test registered feature type test:spikes twice"));
        assert!(errors[1].contains("test registered feature type \"other:spikes\""));
        assert!(errors[2].contains("test registered a feature type with no name"));
        assert!(errors[3].contains("test registered feature type \"test:Bad Name\""));
    }

    /// The game's own namespace stays the game's.
    #[test]
    fn the_game_namespace_refuses_feature_types() {
        struct Own;
        #[mod_api::__subscriptions]
        impl Hooks for Own {
            fn register(&self, r: &mut dyn Registrar, _: &mod_api::RegistrationContext) {
                r.register_feature_type("ore", Column);
            }
        }
        let errors = install_parts(
            &[ModEntry::new("minecraft", Box::new(Own))],
            &crate::parts::tests::rule_schema(),
        )
        .unwrap_err();
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("minecraft registered feature type minecraft:ore"));
    }

    /// The engine's region and random reach the mod's code through the API's
    /// own handles, and what the mod writes reaches the engine's region.
    #[test]
    fn a_registered_feature_type_places_through_the_engines_region() {
        use horizon_world::registry::BlockStateId;
        use horizon_world::worldgen::mod_features::{ModFeatureRandom, ModFeatureWorld};
        struct Registers;
        #[mod_api::__subscriptions]
        impl Hooks for Registers {
            fn register(&self, r: &mut dyn Registrar, _: &mod_api::RegistrationContext) {
                r.register_feature_type("column", Column);
            }
        }
        struct Region(std::collections::BTreeMap<[i32; 3], BlockStateId>);
        impl ModFeatureWorld for Region {
            fn min_y(&mut self) -> i32 {
                0
            }
            fn max_y(&mut self) -> i32 {
                15
            }
            fn block(&mut self, p: [i32; 3]) -> Option<BlockStateId> {
                Some(self.0.get(&p).copied().unwrap_or(BlockStateId(0)))
            }
            fn set_block(&mut self, p: [i32; 3], s: BlockStateId, _: i32) -> bool {
                self.0.insert(p, s);
                true
            }
            fn height(
                &mut self,
                _: horizon_world::heightmap::HeightmapType,
                _: i32,
                _: i32,
            ) -> Option<i32> {
                None
            }
            fn state(&mut self, name: &str, _: &[(&str, &str)]) -> Option<BlockStateId> {
                (name == "gold_block").then_some(BlockStateId(9))
            }
            fn is_air(&mut self, s: BlockStateId) -> bool {
                s == BlockStateId(0)
            }
            fn block_in_tag(&mut self, _: [i32; 3], _: &str) -> bool {
                false
            }
        }
        struct Fixed;
        impl ModFeatureRandom for Fixed {
            fn next_int_bound(&mut self, _: i32) -> i32 {
                2
            }
            fn next_int(&mut self) -> i32 {
                0
            }
            fn next_long(&mut self) -> i64 {
                0
            }
            fn next_float(&mut self) -> f32 {
                0.0
            }
            fn next_double(&mut self) -> f64 {
                0.0
            }
            fn next_boolean(&mut self) -> bool {
                false
            }
        }
        install_parts(
            &[ModEntry::new("test", Box::new(Registers))],
            &crate::parts::tests::rule_schema(),
        )
        .unwrap();
        let types = mod_api::__private::installed_feature_types();
        let feature = types["test:column"]
            .decode(&serde_json::json!({"type": "test:column", "height": 2}))
            .unwrap();
        let mut region = Region(Default::default());
        assert!(feature.place(&mut region, &mut Fixed, [10, 4, 7]));
        assert_eq!(
            region.0.into_iter().collect::<Vec<_>>(),
            [([12, 4, 7], BlockStateId(9)), ([12, 5, 7], BlockStateId(9))]
        );
        assert!(types["test:column"].decode(&serde_json::json!({})).is_err());
    }
}
