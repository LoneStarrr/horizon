//! Native tree parts: the mod's trunk placers, foliage placers and decorators behind the
//! engine's tree feature.
use horizon_world::registry::BlockStateId;
use horizon_world::worldgen::mod_tree_parts::{
    ModFoliageAttachment, ModFoliagePlacer, ModFoliagePlacerType, ModTreeContext, ModTreeDecorator,
    ModTreeDecoratorType, ModTreeParts, ModTreeRandom, ModTrunkPlacer, ModTrunkPlacerType,
    TreeAxis as EngineAxis, install_mod_tree_parts,
};
use mod_api::{
    FoliageAttachment, FoliagePlacer, FoliagePlacerType, TreeAxis, TreeContext, TreeDecorator,
    TreeDecoratorType, TreeRandom, TreeState, TrunkPlacer, TrunkPlacerType,
};
use std::sync::Arc;

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
    TreeState::from_raw(id.0)
}
fn engine_state(state: TreeState) -> BlockStateId {
    BlockStateId(state.raw())
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

struct TrunkType(Arc<dyn TrunkPlacerType>);
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

struct FoliageType(Arc<dyn FoliagePlacerType>);
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

struct DecoratorType(Arc<dyn TreeDecoratorType>);
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

/// The tree parts the mods registered at startup, as the engine reads them: `modid:name` ->
/// type, by kind. Empty before the parts are installed.
pub(in crate::game) fn installed_tree_parts() -> ModTreeParts {
    let declared = mod_api::__private::tree_part_declarations();
    ModTreeParts {
        trunk_placers: declared
            .trunk_placers
            .into_iter()
            .map(|(id, ty)| (id, Arc::new(TrunkType(ty)) as Arc<dyn ModTrunkPlacerType>))
            .collect(),
        foliage_placers: declared
            .foliage_placers
            .into_iter()
            .map(|(id, ty)| {
                (
                    id,
                    Arc::new(FoliageType(ty)) as Arc<dyn ModFoliagePlacerType>,
                )
            })
            .collect(),
        decorators: declared
            .decorators
            .into_iter()
            .map(|(id, ty)| {
                (
                    id,
                    Arc::new(DecoratorType(ty)) as Arc<dyn ModTreeDecoratorType>,
                )
            })
            .collect(),
    }
}

/// Hand the tree parts the mods registered to the engine, before any world opens.
pub(in crate::game) fn install() {
    if install_mod_tree_parts(installed_tree_parts()).is_err() {
        log::warn!("Mod tree parts already installed; keeping the first table");
    }
}

#[cfg(test)]
#[path = "native_tree_parts/tests.rs"]
mod tests;
