//! Tree parts a native mod registers for vanilla's `minecraft:tree` feature: trunk placer,
//! foliage placer and tree decorator types, as
//! [`register_trunk_placer`](super::Registrar::register_trunk_placer),
//! [`register_foliage_placer`](super::Registrar::register_foliage_placer) and
//! [`register_tree_decorator`](super::Registrar::register_tree_decorator) hand them over once at
//! startup (NeoForge's registries `TRUNK_PLACER_TYPE`, `FOLIAGE_PLACER_TYPE` and
//! `TREE_DECORATOR_TYPE`). A data pack's tree config naming `"type": "mymod:spiral_trunk_placer"`
//! then places by the mod's code, inside the game's own tree feature: the tree height, foliage
//! height and radius are drawn in vanilla's order around the part, the minimum-size clipping,
//! decorators, leaf distances and shape updates stay the game's.
//!
//! The contracts mirror vanilla's `TrunkPlacer.placeTrunk`, `FoliagePlacer.foliageHeight`,
//! `foliageRadius` and `createFoliage`, and `TreeDecorator.place`. A type decodes its own JSON
//! config. The engine reads the fields vanilla's codecs share: `base_height`, `height_rand_a` and
//! `height_rand_b` of a trunk placer, `radius` and `offset` of a foliage placer.
//!
//! There is no `Host` here: trees grow on the worldgen worker threads, so a part sees only the
//! [`TreeContext`] of the tree being placed, is `Send + Sync`, and keeps no mutable state.
use horizon_world::registry::BlockStateId;
use horizon_world::worldgen::mod_tree_parts::{
    ModFoliageAttachment, ModFoliagePlacer, ModFoliagePlacerType, ModTreeContext, ModTreeDecorator,
    ModTreeDecoratorType, ModTreeParts, ModTreeRandom, ModTrunkPlacer, ModTrunkPlacerType,
    TreeAxis as EngineAxis,
};
use std::sync::Arc;

/// A block state a tree part places: the handle [`TreeContext::state`] and
/// [`TreeContext::block`] give. It means something only inside the tree that gave it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct TreeState(pub(crate) u32);

/// A log's axis (`RotatedPillarBlock.AXIS`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: a log has exactly these three axes
pub enum TreeAxis {
    /// East-west.
    X,
    /// Up-down.
    Y,
    /// North-south.
    Z,
}

/// `FoliageAttachment`: where a trunk placer asks the foliage placer to grow leaves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: mods build it themselves as place_trunk's answer
pub struct FoliageAttachment {
    /// The attachment's block position.
    pub pos: [i32; 3],
    /// `radiusOffset`: added to the leaf radius.
    pub radius_offset: i32,
    /// `doubleTrunk`: the attachment sits on a 2x2 trunk.
    pub double_trunk: bool,
}

/// The tree's random, in vanilla's draw order.
pub trait TreeRandom {
    /// `nextInt(bound)`; 0 for a bound below 1.
    fn next_int_bound(&mut self, bound: i32) -> i32;
    /// `nextLong`.
    fn next_long(&mut self) -> i64;
    /// `nextFloat`.
    fn next_float(&mut self) -> f32;
    /// `nextDouble`.
    fn next_double(&mut self) -> f64;
    /// `nextBoolean`.
    fn next_boolean(&mut self) -> bool;
}

/// What a part sees of the tree being placed: vanilla's `LevelSimulatedReader` checks and the
/// setters of the part's phase. Positions are world blocks. A setter does nothing outside its
/// phase: logs in a trunk placer, leaves in a foliage placer, decorations in a decorator.
pub trait TreeContext: TreeRandom {
    /// The default state of block `name` (`oak_log`, `minecraft:vine`), `None` for a block the
    /// game doesn't have.
    fn state(&self, name: &str) -> Option<TreeState>;
    /// `state` with property `name` set to `value`, `None` for a property it lacks.
    fn with_property(&self, state: TreeState, name: &str, value: &str) -> Option<TreeState>;
    /// The block at `pos`; `None` where the level can't be read.
    fn block(&mut self, pos: [i32; 3]) -> Option<TreeState>;
    /// `BlockState.isAir` at `pos`.
    fn is_air(&mut self, pos: [i32; 3]) -> bool;
    /// `TreeFeature.validTreePos`: air, or a block trees replace.
    fn valid_tree_pos(&mut self, pos: [i32; 3]) -> bool;
    /// `TrunkPlacer.isFree`: a valid tree position or a log.
    fn is_free(&mut self, pos: [i32; 3]) -> bool;
    /// `TrunkPlacer.placeLog`: the tree's trunk state at `pos`, turned to `axis` where the block
    /// has one, if `pos` is a valid tree position. Answers whether it was placed. Trunk placers.
    fn place_log(&mut self, pos: [i32; 3], axis: Option<TreeAxis>) -> bool;
    /// `TrunkPlacer.placeLogIfFree`. Trunk placers.
    fn place_log_if_free(&mut self, pos: [i32; 3]);
    /// `TrunkPlacer.setDirtAt`: the tree's below-trunk state at `pos`, if it has one. Trunk
    /// placers.
    fn place_below_trunk(&mut self, pos: [i32; 3]);
    /// `FoliagePlacer.tryPlaceLeaf`: the tree's foliage state at `pos`, unless a persistent
    /// block or a position trees can't grow into is there. Answers whether it was placed.
    /// Foliage placers.
    fn try_place_leaf(&mut self, pos: [i32; 3]) -> bool;
    /// `FoliagePlacer.offset(random)`: one draw of the config's `offset`. Foliage placers; 0
    /// elsewhere.
    fn foliage_offset(&mut self) -> i32;
    /// `TreeDecorator.Context.setBlock`: write `state` at `pos` as a decoration, answering whether
    /// it was written. Decorators.
    fn set_decoration(&mut self, pos: [i32; 3], state: TreeState) -> bool;
}

/// A mod's trunk placer type: vanilla's `TrunkPlacerType`. Implemented once, kept by the game.
pub trait TrunkPlacerType: Send + Sync + 'static {
    /// Decode one use from its JSON object, `type` field included, or say why it is invalid
    /// (the message names the type in the load error).
    fn decode(&self, config: &serde_json::Value) -> Result<Arc<dyn TrunkPlacer>, String>;
}

/// One decoded use of a trunk placer type.
pub trait TrunkPlacer: std::fmt::Debug + Send + Sync {
    /// `TrunkPlacer.placeTrunk`: place the trunk at `origin` for `tree_height` (already clipped
    /// to the free height) and answer where leaves attach.
    fn place_trunk(
        &self,
        ctx: &mut dyn TreeContext,
        tree_height: i32,
        origin: [i32; 3],
    ) -> Vec<FoliageAttachment>;
}

/// A mod's foliage placer type: vanilla's `FoliagePlacerType`.
pub trait FoliagePlacerType: Send + Sync + 'static {
    /// Decode one use from its JSON object, `type` field included, or say why it is invalid.
    fn decode(&self, config: &serde_json::Value) -> Result<Arc<dyn FoliagePlacer>, String>;
}

/// One decoded use of a foliage placer type.
pub trait FoliagePlacer: std::fmt::Debug + Send + Sync {
    /// `FoliagePlacer.foliageHeight`.
    fn foliage_height(&self, random: &mut dyn TreeRandom, tree_height: i32) -> i32;
    /// `FoliagePlacer.foliageRadius`: `radius` is the config's `radius`, drawn already as
    /// vanilla's default does; return it, or adjust it with further draws.
    fn foliage_radius(&self, random: &mut dyn TreeRandom, trunk_height: i32, radius: i32) -> i32 {
        let _ = (random, trunk_height);
        radius
    }
    /// `FoliagePlacer.createFoliage` for one attachment.
    fn create_foliage(
        &self,
        ctx: &mut dyn TreeContext,
        tree_height: i32,
        attachment: FoliageAttachment,
        foliage_height: i32,
        leaf_radius: i32,
    );
}

/// A mod's tree decorator type: vanilla's `TreeDecoratorType`.
pub trait TreeDecoratorType: Send + Sync + 'static {
    /// Decode one use from its JSON object, `type` field included, or say why it is invalid.
    fn decode(&self, config: &serde_json::Value) -> Result<Arc<dyn TreeDecorator>, String>;
}

/// One decoded use of a tree decorator type.
pub trait TreeDecorator: std::fmt::Debug + Send + Sync {
    /// `TreeDecorator.place`: the tree's logs, leaves and roots as placed, each by ascending Y.
    fn place(
        &self,
        ctx: &mut dyn TreeContext,
        logs: &[[i32; 3]],
        leaves: &[[i32; 3]],
        roots: &[[i32; 3]],
    );
}

/// The engine's context, as a [`TreeContext`].
struct Ctx<'a>(&'a mut dyn ModTreeContext);

/// The engine's bare random, as a [`TreeRandom`].
struct Draws<'a>(&'a mut dyn ModTreeRandom);

macro_rules! random_of {
    ($ty:ty) => {
        impl TreeRandom for $ty {
            fn next_int_bound(&mut self, bound: i32) -> i32 {
                self.0.next_int_bound(bound)
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
    };
}
random_of!(Ctx<'_>);
random_of!(Draws<'_>);

fn state(id: BlockStateId) -> TreeState {
    TreeState(id.0)
}
fn engine_state(state: TreeState) -> BlockStateId {
    BlockStateId(state.0)
}

impl TreeContext for Ctx<'_> {
    fn state(&self, name: &str) -> Option<TreeState> {
        self.0.state(name).map(state)
    }
    fn with_property(&self, from: TreeState, name: &str, value: &str) -> Option<TreeState> {
        self.0
            .with_property(engine_state(from), name, value)
            .map(state)
    }
    fn block(&mut self, pos: [i32; 3]) -> Option<TreeState> {
        self.0.block(pos).map(state)
    }
    fn is_air(&mut self, pos: [i32; 3]) -> bool {
        self.0.is_air(pos)
    }
    fn valid_tree_pos(&mut self, pos: [i32; 3]) -> bool {
        self.0.valid_tree_pos(pos)
    }
    fn is_free(&mut self, pos: [i32; 3]) -> bool {
        self.0.is_free(pos)
    }
    fn place_log(&mut self, pos: [i32; 3], axis: Option<TreeAxis>) -> bool {
        self.0.place_log(
            pos,
            axis.map(|a| match a {
                TreeAxis::X => EngineAxis::X,
                TreeAxis::Y => EngineAxis::Y,
                TreeAxis::Z => EngineAxis::Z,
            }),
        )
    }
    fn place_log_if_free(&mut self, pos: [i32; 3]) {
        self.0.place_log_if_free(pos);
    }
    fn place_below_trunk(&mut self, pos: [i32; 3]) {
        self.0.place_below_trunk(pos);
    }
    fn try_place_leaf(&mut self, pos: [i32; 3]) -> bool {
        self.0.try_place_leaf(pos)
    }
    fn foliage_offset(&mut self) -> i32 {
        self.0.foliage_offset()
    }
    fn set_decoration(&mut self, pos: [i32; 3], at: TreeState) -> bool {
        self.0.set_decoration(pos, engine_state(at))
    }
}

pub(crate) struct TrunkType(pub(crate) Arc<dyn TrunkPlacerType>);
#[derive(Debug)]
struct TrunkInstance(Arc<dyn TrunkPlacer>);

impl ModTrunkPlacerType for TrunkType {
    fn decode(&self, config: &serde_json::Value) -> Result<Arc<dyn ModTrunkPlacer>, String> {
        Ok(Arc::new(TrunkInstance(self.0.decode(config)?)))
    }
}
impl ModTrunkPlacer for TrunkInstance {
    fn place_trunk(
        &self,
        ctx: &mut dyn ModTreeContext,
        tree_height: i32,
        origin: [i32; 3],
    ) -> Vec<ModFoliageAttachment> {
        self.0
            .place_trunk(&mut Ctx(ctx), tree_height, origin)
            .into_iter()
            .map(|a| ModFoliageAttachment {
                pos: a.pos,
                radius_offset: a.radius_offset,
                double_trunk: a.double_trunk,
            })
            .collect()
    }
}

pub(crate) struct FoliageType(pub(crate) Arc<dyn FoliagePlacerType>);
#[derive(Debug)]
struct FoliageInstance(Arc<dyn FoliagePlacer>);

impl ModFoliagePlacerType for FoliageType {
    fn decode(&self, config: &serde_json::Value) -> Result<Arc<dyn ModFoliagePlacer>, String> {
        Ok(Arc::new(FoliageInstance(self.0.decode(config)?)))
    }
}
impl ModFoliagePlacer for FoliageInstance {
    fn foliage_height(&self, random: &mut dyn ModTreeRandom, tree_height: i32) -> i32 {
        self.0.foliage_height(&mut Draws(random), tree_height)
    }
    fn foliage_radius(
        &self,
        random: &mut dyn ModTreeRandom,
        trunk_height: i32,
        radius: i32,
    ) -> i32 {
        self.0
            .foliage_radius(&mut Draws(random), trunk_height, radius)
    }
    fn create_foliage(
        &self,
        ctx: &mut dyn ModTreeContext,
        tree_height: i32,
        attachment: ModFoliageAttachment,
        foliage_height: i32,
        leaf_radius: i32,
    ) {
        self.0.create_foliage(
            &mut Ctx(ctx),
            tree_height,
            FoliageAttachment {
                pos: attachment.pos,
                radius_offset: attachment.radius_offset,
                double_trunk: attachment.double_trunk,
            },
            foliage_height,
            leaf_radius,
        );
    }
}

pub(crate) struct DecoratorType(pub(crate) Arc<dyn TreeDecoratorType>);
#[derive(Debug)]
struct DecoratorInstance(Arc<dyn TreeDecorator>);

impl ModTreeDecoratorType for DecoratorType {
    fn decode(&self, config: &serde_json::Value) -> Result<Arc<dyn ModTreeDecorator>, String> {
        Ok(Arc::new(DecoratorInstance(self.0.decode(config)?)))
    }
}
impl ModTreeDecorator for DecoratorInstance {
    fn place(
        &self,
        ctx: &mut dyn ModTreeContext,
        logs: &[[i32; 3]],
        leaves: &[[i32; 3]],
        roots: &[[i32; 3]],
    ) {
        self.0.place(&mut Ctx(ctx), logs, leaves, roots);
    }
}

/// The tree parts the mods registered, by kind, as engine types.
#[derive(Default)]
pub(crate) struct TreePartTable {
    pub(crate) trunk_placers: Vec<(Box<str>, Arc<dyn ModTrunkPlacerType>)>,
    pub(crate) foliage_placers: Vec<(Box<str>, Arc<dyn ModFoliagePlacerType>)>,
    pub(crate) decorators: Vec<(Box<str>, Arc<dyn ModTreeDecoratorType>)>,
}

/// The tree parts the mods registered at startup, as the engine reads them:
/// `modid:name` → type, by kind. Empty before the parts are installed.
pub fn installed_tree_parts() -> ModTreeParts {
    let table = super::parts::installed_tree_part_table();
    ModTreeParts {
        trunk_placers: table
            .map(|t| &t.trunk_placers)
            .into_iter()
            .flatten()
            .map(|(id, ty)| (id.to_string(), ty.clone()))
            .collect(),
        foliage_placers: table
            .map(|t| &t.foliage_placers)
            .into_iter()
            .flatten()
            .map(|(id, ty)| (id.to_string(), ty.clone()))
            .collect(),
        decorators: table
            .map(|t| &t.decorators)
            .into_iter()
            .flatten()
            .map(|(id, ty)| (id.to_string(), ty.clone()))
            .collect(),
    }
}
