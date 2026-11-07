//! A feature type a mod registers decodes inside a placed feature and runs on
//! a small fake region, with the decorator's random.
use horizon_core::random::{LegacyRandom, RandomSource};
use horizon_world::{
    heightmap::HeightmapType,
    registry::{BlockRegistry, BlockStateId},
    worldgen::{
        anchor::WorldGenerationContext,
        feature::{FeatureBlocks, FeatureWorld},
        feature_graph::PlacedFeatureDefinition,
        mod_features::{
            ModFeature, ModFeatureRandom, ModFeatureType, ModFeatureTypes, ModFeatureWorld,
            install_mod_feature_types,
        },
        placement::{PlacementContext, PlacementQueryError},
    },
};
use std::{collections::BTreeMap, sync::Arc};

const AIR: BlockStateId = BlockStateId(0);
const GOLD: BlockStateId = BlockStateId(1);

/// A column of `state` as tall as `height`, at an x the random picks.
#[derive(Debug)]
struct Column {
    height: i32,
}
impl ModFeature for Column {
    fn place(
        &self,
        world: &mut dyn ModFeatureWorld,
        random: &mut dyn ModFeatureRandom,
        origin: [i32; 3],
    ) -> bool {
        let Some(gold) = world.state("gold_block", &[]) else {
            return false;
        };
        let x = origin[0] + random.next_int_bound(4);
        let mut placed = false;
        for dy in 0..self.height {
            let at = [x, origin[1] + dy, origin[2]];
            if world.block(at).is_some_and(|s| world.is_air(s)) {
                placed |= world.set_block(at, gold, 2);
            }
        }
        placed
    }
}
struct ColumnType;
impl ModFeatureType for ColumnType {
    fn decode(&self, config: &serde_json::Value) -> Result<Arc<dyn ModFeature>, String> {
        let height = config["height"].as_i64().ok_or("missing height")?;
        Ok(Arc::new(Column {
            height: height as i32,
        }))
    }
}

struct World {
    blocks: BTreeMap<[i32; 3], BlockStateId>,
    registry: BlockRegistry,
}
impl World {
    fn new() -> Self {
        let row = |name: &str, air: bool| serde_json::json!([name, air, false, "", {"id": name, "properties": {}}]);
        let facts = serde_json::json!({
            "defaults": {"minecraft:air": 0, "minecraft:gold_block": 1},
            "states": [row("minecraft:air", true), row("minecraft:gold_block", false)]});
        Self {
            blocks: BTreeMap::new(),
            registry: BlockRegistry::from_facts_json(facts.to_string().as_bytes()).unwrap(),
        }
    }
}
impl PlacementContext for World {
    fn generation(&self) -> WorldGenerationContext {
        WorldGenerationContext::new(0, 16, 8)
    }
    fn min_y(&mut self) -> Result<i32, PlacementQueryError> {
        Ok(0)
    }
    fn max_y(&mut self) -> Result<i32, PlacementQueryError> {
        Ok(15)
    }
    fn height(&mut self, _: HeightmapType, _: i32, _: i32) -> Result<i32, PlacementQueryError> {
        Ok(0)
    }
    fn block_registry(&self) -> Option<&BlockRegistry> {
        Some(&self.registry)
    }
}
impl FeatureWorld for World {
    type Error = std::convert::Infallible;
    fn block(&mut self, p: [i32; 3]) -> Result<BlockStateId, Self::Error> {
        Ok(self.blocks.get(&p).copied().unwrap_or(AIR))
    }
    fn set_block(&mut self, p: [i32; 3], s: BlockStateId, _: i32) -> Result<bool, Self::Error> {
        self.blocks.insert(p, s);
        Ok(true)
    }
}
fn blocks() -> FeatureBlocks {
    FeatureBlocks {
        air: AIR,
        stone: BlockStateId(1),
        cobblestone: BlockStateId(2),
        end_stone: BlockStateId(3),
        obsidian: BlockStateId(4),
    }
}

#[test]
fn a_registered_feature_type_decodes_and_runs_in_a_placed_feature() {
    let mut types = ModFeatureTypes::new();
    types.insert("test:column".into(), Arc::new(ColumnType));
    install_mod_feature_types(types).ok().unwrap();

    let placed = PlacedFeatureDefinition::from_json(
        r#"{"feature":{"type":"test:column","height":3},"placement":[]}"#,
    )
    .unwrap();
    let mut world = World::new();
    // The first draw of the decorator's stream picks the column's x.
    let mut expected = LegacyRandom::new(9);
    let x = 10 + expected.next_int_bound(4).unwrap();
    let mut random = LegacyRandom::new(9);
    assert!(
        placed
            .place(&mut world, &mut random, [10, 5, 20], blocks())
            .unwrap()
    );
    let column: Vec<_> = (5..8).map(|y| world.blocks[&[x, y, 20]]).collect();
    assert_eq!(column, [GOLD; 3]);
    assert_eq!(world.blocks.len(), 3);
    assert_eq!(random.next_long(), expected.next_long());

    // Blocks already there stay: a second run over the column places nothing.
    let mut again = LegacyRandom::new(9);
    assert!(
        !placed
            .place(&mut world, &mut again, [10, 5, 20], blocks())
            .unwrap()
    );

    // A config the type refuses fails the placed feature's decoding.
    assert!(
        PlacedFeatureDefinition::from_json(r#"{"feature":{"type":"test:column"},"placement":[]}"#)
            .is_err()
    );
    // A type nobody registered is still unknown.
    assert!(
        PlacedFeatureDefinition::from_json(
            r#"{"feature":{"type":"test:nothing","height":1},"placement":[]}"#
        )
        .is_err()
    );
}
