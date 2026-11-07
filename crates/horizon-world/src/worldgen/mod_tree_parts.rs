//! Tree parts a mod registers for `minecraft:tree`: trunk placer, foliage placer and tree decorator
//! types, handed over once at startup and looked up when a tree config's `type` names one the
//! engine doesn't know (`"type": "mymod:spiral_trunk_placer"`). The contracts mirror vanilla's
//! `TrunkPlacer.placeTrunk`, `FoliagePlacer.foliageHeight`/`foliageRadius`/`createFoliage` and
//! `TreeDecorator.place`: the part's own code runs in the tree feature's place, on the engine's
//! random and through the [`ModTreeContext`] writes, so the draws around it stay vanilla's.
//! Parts run on the worldgen worker threads and keep no mutable state.
use crate::registry::BlockStateId;
use std::collections::BTreeMap;
use std::fmt::Debug;
use std::sync::{Arc, OnceLock};

/// A log's axis (`RotatedPillarBlock.AXIS`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TreeAxis {
    X,
    Y,
    Z,
}

/// `FoliageAttachment`: where a trunk placer wants a foliage placer to grow leaves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ModFoliageAttachment {
    pub pos: [i32; 3],
    pub radius_offset: i32,
    /// `doubleTrunk`: the attachment sits on a 2x2 trunk.
    pub double_trunk: bool,
}

/// The feature's random, in vanilla's draw order.
pub trait ModTreeRandom {
    /// `nextInt(bound)`; 0 for a bound below 1.
    fn next_int_bound(&mut self, bound: i32) -> i32;
    fn next_long(&mut self) -> i64;
    fn next_float(&mut self) -> f32;
    fn next_double(&mut self) -> f64;
    fn next_boolean(&mut self) -> bool;
}

/// What a part sees of the tree being placed: vanilla's `LevelSimulatedReader` checks and the
/// setters (`BiConsumer<BlockPos, BlockState>`) of the current phase. A phase's setters do
/// nothing outside it: logs and below-trunk blocks in a trunk placer, leaves in a foliage placer,
/// decorations in a decorator.
pub trait ModTreeContext: ModTreeRandom {
    /// The default state of block `name` (`oak_log`, `minecraft:vine`).
    fn state(&self, name: &str) -> Option<BlockStateId>;
    /// `state` with property `name` set to `value`, `None` for a property it lacks.
    fn with_property(&self, state: BlockStateId, name: &str, value: &str) -> Option<BlockStateId>;
    /// The block at `pos`; `None` where the level can't be read.
    fn block(&mut self, pos: [i32; 3]) -> Option<BlockStateId>;
    /// `BlockState.isAir` at `pos`.
    fn is_air(&mut self, pos: [i32; 3]) -> bool;
    /// `TreeFeature.validTreePos`: air or replaceable by trees.
    fn valid_tree_pos(&mut self, pos: [i32; 3]) -> bool;
    /// `TrunkPlacer.isFree`: a valid tree position or a log.
    fn is_free(&mut self, pos: [i32; 3]) -> bool;
    /// `TrunkPlacer.placeLog`: the trunk provider's state at `pos`, with `axis` where the block
    /// has one, if `pos` is a valid tree position. Answers whether it was placed.
    fn place_log(&mut self, pos: [i32; 3], axis: Option<TreeAxis>) -> bool;
    /// `TrunkPlacer.placeLogIfFree`.
    fn place_log_if_free(&mut self, pos: [i32; 3]);
    /// `TrunkPlacer.setDirtAt`: the below-trunk provider's state at `pos`, if it gives one.
    fn place_below_trunk(&mut self, pos: [i32; 3]);
    /// `FoliagePlacer.tryPlaceLeaf`: the foliage provider's state at `pos`, unless a persistent
    /// block or a position trees can't grow into is there. Answers whether it was placed.
    fn try_place_leaf(&mut self, pos: [i32; 3]) -> bool;
    /// `FoliagePlacer.offset(random)`: one draw of the config's `offset`. 0 outside a foliage placer.
    fn foliage_offset(&mut self) -> i32;
    /// `TreeDecorator.Context.setBlock`: write `state` at `pos` as a decoration. Answers whether
    /// it was written.
    fn set_decoration(&mut self, pos: [i32; 3], state: BlockStateId) -> bool;
}

/// A trunk placer type's instance, its config decoded.
pub trait ModTrunkPlacer: Debug + Send + Sync {
    /// `TrunkPlacer.placeTrunk`: place the trunk at `origin` for `tree_height` (already clipped
    /// to the free height) and answer where leaves attach.
    fn place_trunk(
        &self,
        ctx: &mut dyn ModTreeContext,
        tree_height: i32,
        origin: [i32; 3],
    ) -> Vec<ModFoliageAttachment>;
}

/// A trunk placer type: vanilla's `TrunkPlacerType`. The engine reads `base_height`,
/// `height_rand_a` and `height_rand_b` itself, as vanilla's `trunkPlacerParts` do.
pub trait ModTrunkPlacerType: Send + Sync {
    /// Decode one use from its JSON object (the `type` field included).
    fn decode(&self, config: &serde_json::Value) -> Result<Arc<dyn ModTrunkPlacer>, String>;
}

/// A foliage placer type's instance, its config decoded.
pub trait ModFoliagePlacer: Debug + Send + Sync {
    /// `FoliagePlacer.foliageHeight`.
    fn foliage_height(&self, random: &mut dyn ModTreeRandom, tree_height: i32) -> i32;
    /// `FoliagePlacer.foliageRadius`: `radius` is the config's `radius` already sampled, as
    /// vanilla's default draws it; return it, or adjust it with more draws.
    fn foliage_radius(
        &self,
        random: &mut dyn ModTreeRandom,
        trunk_height: i32,
        radius: i32,
    ) -> i32 {
        let _ = (random, trunk_height);
        radius
    }
    /// `FoliagePlacer.createFoliage` for one attachment.
    fn create_foliage(
        &self,
        ctx: &mut dyn ModTreeContext,
        tree_height: i32,
        attachment: ModFoliageAttachment,
        foliage_height: i32,
        leaf_radius: i32,
    );
}

/// A foliage placer type: vanilla's `FoliagePlacerType`. The engine reads `radius` and `offset`.
pub trait ModFoliagePlacerType: Send + Sync {
    fn decode(&self, config: &serde_json::Value) -> Result<Arc<dyn ModFoliagePlacer>, String>;
}

/// A tree decorator type's instance, its config decoded.
pub trait ModTreeDecorator: Debug + Send + Sync {
    /// `TreeDecorator.place`: the placed logs and leaves (vanilla's order: by Y) and roots.
    fn place(
        &self,
        ctx: &mut dyn ModTreeContext,
        logs: &[[i32; 3]],
        leaves: &[[i32; 3]],
        roots: &[[i32; 3]],
    );
}

/// A tree decorator type: vanilla's `TreeDecoratorType`.
pub trait ModTreeDecoratorType: Send + Sync {
    fn decode(&self, config: &serde_json::Value) -> Result<Arc<dyn ModTreeDecorator>, String>;
}

/// The installed types, `modid:name` → type.
#[derive(Default)]
pub struct ModTreeParts {
    pub trunk_placers: BTreeMap<String, Arc<dyn ModTrunkPlacerType>>,
    pub foliage_placers: BTreeMap<String, Arc<dyn ModFoliagePlacerType>>,
    pub decorators: BTreeMap<String, Arc<dyn ModTreeDecoratorType>>,
}

static MOD_TREE_PARTS: OnceLock<ModTreeParts> = OnceLock::new();
static EMPTY: OnceLock<ModTreeParts> = OnceLock::new();

/// Install the process-wide table once; a second installation keeps the first.
pub fn install_mod_tree_parts(parts: ModTreeParts) -> Result<(), ModTreeParts> {
    MOD_TREE_PARTS.set(parts)
}

/// The installed parts: empty until the executable installs the mods'.
pub fn mod_tree_parts() -> &'static ModTreeParts {
    MOD_TREE_PARTS
        .get()
        .unwrap_or_else(|| EMPTY.get_or_init(ModTreeParts::default))
}
