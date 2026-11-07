//! Feature types a mod registers (vanilla's `Feature`), handed over once at
//! startup and looked up when a configured feature's JSON names a type the
//! engine doesn't know (`"type": "mymod:spikes"`). The decoded feature joins
//! the feature graph as a node, so placement modifiers, decoration steps and
//! the per-feature seeding treat it like any other. Runs on the worldgen
//! worker threads, so a type and its features keep no mutable state.
use super::feature::{FeatureBlocks, FeatureWorld};
use super::placement::PlacementContext;
use crate::heightmap::HeightmapType;
use crate::registry::BlockStateId;
use horizon_core::random::RandomSource;
use std::collections::BTreeMap;
use std::sync::{Arc, OnceLock};

/// A feature type's engine side.
pub trait ModFeatureType: Send + Sync {
    /// Decode one use of the type from its configured feature's JSON object
    /// (the `type` field included), or say why it is invalid. The same JSON
    /// gives the same feature.
    fn decode(&self, config: &serde_json::Value) -> Result<Arc<dyn ModFeature>, String>;
}

/// One decoded use of a feature type: vanilla's `Feature.place` with its
/// configuration bound.
pub trait ModFeature: std::fmt::Debug + Send + Sync {
    /// Place the feature at `origin`, drawing from `random` in the order the
    /// decorator seeded it; `true` when something was placed.
    fn place(
        &self,
        world: &mut dyn ModFeatureWorld,
        random: &mut dyn ModFeatureRandom,
        origin: [i32; 3],
    ) -> bool;
}

/// What a mod feature sees of the region it decorates (vanilla's
/// `WorldGenLevel`). Reads and writes outside the region being generated, or
/// outside the build height, answer `None` or `false`.
pub trait ModFeatureWorld {
    /// The level's lowest block Y.
    fn min_y(&mut self) -> i32;
    /// The level's highest block Y.
    fn max_y(&mut self) -> i32;
    /// The block state at `position`.
    fn block(&mut self, position: [i32; 3]) -> Option<BlockStateId>;
    /// `setBlock(position, state, flags)`; whether it was written.
    fn set_block(&mut self, position: [i32; 3], state: BlockStateId, flags: i32) -> bool;
    /// The `kind` heightmap's value of the column.
    fn height(&mut self, kind: HeightmapType, x: i32, z: i32) -> Option<i32>;
    /// A block state by block name with properties (missing properties keep
    /// their defaults), or `None` for a block the game doesn't have.
    fn state(&mut self, name: &str, properties: &[(&str, &str)]) -> Option<BlockStateId>;
    /// Whether `state` is air.
    fn is_air(&mut self, state: BlockStateId) -> bool;
    /// Whether the block at `position` is in the block tag `tag`.
    fn block_in_tag(&mut self, position: [i32; 3], tag: &str) -> bool;
}

/// The feature's random, in vanilla's order.
pub trait ModFeatureRandom {
    /// `nextInt(bound)`; 0 for a bound below 1.
    fn next_int_bound(&mut self, bound: i32) -> i32;
    /// `nextInt()`.
    fn next_int(&mut self) -> i32;
    /// `nextLong`.
    fn next_long(&mut self) -> i64;
    /// `nextFloat`.
    fn next_float(&mut self) -> f32;
    /// `nextDouble`.
    fn next_double(&mut self) -> f64;
    /// `nextBoolean`.
    fn next_boolean(&mut self) -> bool;
}

/// The installed types, `modid:spikes` → type. Set once, before any world opens.
pub type ModFeatureTypes = BTreeMap<String, Arc<dyn ModFeatureType>>;

static MOD_FEATURE_TYPES: OnceLock<ModFeatureTypes> = OnceLock::new();
static EMPTY: ModFeatureTypes = BTreeMap::new();

/// Install the process-wide table once; a second installation keeps the first.
pub fn install_mod_feature_types(types: ModFeatureTypes) -> Result<(), ModFeatureTypes> {
    MOD_FEATURE_TYPES.set(types)
}

/// The installed types: empty until the executable installs the mods'.
pub fn mod_feature_types() -> &'static ModFeatureTypes {
    MOD_FEATURE_TYPES.get().unwrap_or(&EMPTY)
}

/// The engine's feature world, as the mod's.
pub(crate) struct Region<'a, W>(pub(crate) &'a mut W, pub(crate) FeatureBlocks);

impl<W: FeatureWorld + PlacementContext> ModFeatureWorld for Region<'_, W> {
    fn min_y(&mut self) -> i32 {
        self.0.min_y().unwrap_or(0)
    }
    fn max_y(&mut self) -> i32 {
        self.0.max_y().unwrap_or(0)
    }
    fn block(&mut self, position: [i32; 3]) -> Option<BlockStateId> {
        if self.0.outside_build_height(position[1]).unwrap_or(true) {
            return None;
        }
        FeatureWorld::block(self.0, position).ok()
    }
    fn set_block(&mut self, position: [i32; 3], state: BlockStateId, flags: i32) -> bool {
        if self.0.outside_build_height(position[1]).unwrap_or(true) {
            return false;
        }
        FeatureWorld::set_block(self.0, position, state, flags).unwrap_or(false)
    }
    fn height(&mut self, kind: HeightmapType, x: i32, z: i32) -> Option<i32> {
        self.0.height(kind, x, z).ok()
    }
    fn state(&mut self, name: &str, properties: &[(&str, &str)]) -> Option<BlockStateId> {
        let registry = self.0.block_registry()?;
        let name = if name.contains(':') {
            name.to_owned()
        } else {
            format!("minecraft:{name}")
        };
        registry.default_state(&name)?;
        Some(super::structure::legacy::block_state(
            registry, &name, properties,
        ))
    }
    fn is_air(&mut self, state: BlockStateId) -> bool {
        match self.0.block_registry() {
            Some(registry) => matches!(
                registry.state(state).map(|s| s.name.as_str()),
                Some("minecraft:air" | "minecraft:cave_air" | "minecraft:void_air")
            ),
            None => state == self.1.air,
        }
    }
    fn block_in_tag(&mut self, position: [i32; 3], tag: &str) -> bool {
        self.0.block_matches_tag(position, tag).unwrap_or(false)
    }
}

/// The engine's feature random, as the mod's.
pub(crate) struct Draws<'a, R>(pub(crate) &'a mut R);

impl<R: RandomSource> ModFeatureRandom for Draws<'_, R> {
    fn next_int_bound(&mut self, bound: i32) -> i32 {
        self.0.next_int_bound(bound).unwrap_or(0)
    }
    fn next_int(&mut self) -> i32 {
        self.0.next_int()
    }
    fn next_long(&mut self) -> i64 {
        self.0.next_long()
    }
    fn next_float(&mut self) -> f32 {
        self.0.next_float()
    }
    fn next_double(&mut self) -> f64 {
        self.0.next_double()
    }
    fn next_boolean(&mut self) -> bool {
        self.0.next_boolean()
    }
}
