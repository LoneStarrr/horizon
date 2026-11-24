//! Feature types a native mod registers, as
//! [`register_feature_type`](crate::Registrar::register_feature_type) hands
//! them over once at startup: vanilla's `Feature` with its own `place`. A data
//! pack's configured feature with `"type": "mymod:spikes"` then runs the mod's
//! code wherever a placed feature puts it, in the decoration step the
//! biome lists it in, with the decorator's seeding.
//!
//! There is no `Host` here: features place on the worldgen worker threads, so
//! the mod sees only the small [`FeatureRegion`] and [`FeatureRandom`] below,
//! `Send + Sync` and deterministic: the same seed gives the same blocks, and a
//! type keeps no mutable state because two workers can decorate neighbouring
//! chunks at once.
use std::sync::Arc;

/// A mod's feature type: vanilla's `Feature` with its codec, for one
/// `modid:spikes` id. Implemented once, kept by the game.
pub trait FeatureType: Send + Sync + 'static {
    /// Decode one use of the type from its configured feature's JSON object,
    /// `type` field included, or say why it is invalid (the message names the
    /// type in the load error).
    fn decode(&self, config: &serde_json::Value) -> Result<Arc<dyn ConfiguredFeature>, String>;
}

/// One decoded use of a feature type, its config validated.
pub trait ConfiguredFeature: std::fmt::Debug + Send + Sync {
    /// `Feature.place`: place the feature at `origin`, drawing from `random`
    /// (seeded as the decorator seeds it), answering whether anything was
    /// placed. The same seed, region and origin give the same blocks.
    fn place(
        &self,
        region: &mut dyn FeatureRegion,
        random: &mut dyn FeatureRandom,
        origin: [i32; 3],
    ) -> bool;
}

/// A block state a feature writes: the handle [`FeatureRegion::state`] and
/// [`FeatureRegion::block`] give and [`FeatureRegion::set_block`] takes. It
/// means something only for the region that gave it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
// no wire: a handle meaningful only to the region that gave it; never sent between processes
#[non_exhaustive]
pub struct FeatureState(pub(crate) u32);

impl FeatureState {
    // hidden: the game's engine adapter converts the handle; mods only receive it.
    #[doc(hidden)]
    pub fn from_raw(raw: u32) -> Self {
        Self(raw)
    }
    // hidden: the game's engine adapter converts the handle; mods only receive it.
    #[doc(hidden)]
    pub fn raw(self) -> u32 {
        self.0
    }
}

/// `Heightmap.Types`: which of a column's heights to read.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
// no wire: a parameter of a feature call into the engine, in process
#[non_exhaustive]
pub enum FeatureHeightmap {
    /// `WORLD_SURFACE_WG`: the top non-air block while the chunk generates.
    WorldSurfaceWg,
    /// `WORLD_SURFACE`: the top non-air block.
    WorldSurface,
    /// `OCEAN_FLOOR_WG`: the top motion-blocking block while the chunk generates.
    OceanFloorWg,
    /// `OCEAN_FLOOR`: the top motion-blocking block.
    OceanFloor,
    /// `MOTION_BLOCKING`: the top block that blocks motion or holds a fluid.
    MotionBlocking,
    /// `MOTION_BLOCKING_NO_LEAVES`: `MotionBlocking` without leaves.
    MotionBlockingNoLeaves,
}

/// The region a feature decorates: vanilla's `WorldGenLevel`, as far as a
/// feature needs it. Positions are world blocks. A read or write outside the
/// region being generated, or outside the build height, answers `None` or
/// `false`.
pub trait FeatureRegion {
    /// `getMinY`: the level's lowest block Y.
    fn min_y(&mut self) -> i32;
    /// `getMaxY`: the level's highest block Y.
    fn max_y(&mut self) -> i32;
    /// `getBlockState`.
    fn block(&mut self, pos: [i32; 3]) -> Option<FeatureState>;
    /// `setBlock(pos, state, flags)` (2 is vanilla's `UPDATE_CLIENTS`),
    /// answering whether it was written.
    fn set_block(&mut self, pos: [i32; 3], state: FeatureState, flags: i32) -> bool;
    /// `getHeight(kind, x, z)`: the column's top by `kind`.
    fn height(&mut self, kind: FeatureHeightmap, x: i32, z: i32) -> Option<i32>;
    /// A block state by block name (`stone`, `minecraft:oak_log`) with
    /// properties (`oak_log` with `axis=y`); missing properties keep their
    /// defaults. `None` for a block the game doesn't have.
    fn state(&mut self, name: &str, properties: &[(&str, &str)]) -> Option<FeatureState>;
    /// `BlockState.isAir`.
    fn is_air(&mut self, state: FeatureState) -> bool;
    /// Whether the block at `pos` is in block tag `tag` (`minecraft:logs`; a
    /// leading `#` is accepted).
    fn block_in_tag(&mut self, pos: [i32; 3], tag: &str) -> bool;
}

/// The random a feature draws from, in vanilla's order: the decorator's
/// feature-seeded `RandomSource`.
pub trait FeatureRandom {
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

/// What the game knows about a registered type: its namespaced id
/// (`modid:spikes`), which a configured feature's `type` names.
pub(crate) struct FeatureTypeInfo {
    pub(crate) id: Box<str>,
    pub(crate) ty: Arc<dyn FeatureType>,
}

/// The types the mods registered at startup: `modid:spikes` → type. Empty before the parts
/// are installed.
// hidden: the game's startup adapter reads the registered types; mods declare them through the registrar.
#[doc(hidden)]
pub fn feature_type_declarations() -> Vec<(String, Arc<dyn FeatureType>)> {
    crate::parts::installed_feature_type_list()
        .iter()
        .map(|info| (info.id.to_string(), info.ty.clone()))
        .collect()
}
