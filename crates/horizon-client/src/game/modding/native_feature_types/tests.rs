use super::*;
use mod_api::{Hooks, ModEntry, Registrar};


/// A column of `height` gold blocks at an x the random picks.
struct Column;
#[derive(Debug)]
struct ColumnFeature(i32);
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
impl mod_api::FeatureType for Column {
    fn decode(
        &self,
        config: &serde_json::Value,
    ) -> Result<Arc<dyn mod_api::ConfiguredFeature>, String> {
        let height = config["height"].as_i64().ok_or("missing height")?;
        Ok(Arc::new(ColumnFeature(height as i32)))
    }
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
    mod_platform::__private::part_tests::install_parts_fixture(&[ModEntry::new("test", Box::new(Registers))])
    .unwrap();
    let types = installed_feature_types();
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
