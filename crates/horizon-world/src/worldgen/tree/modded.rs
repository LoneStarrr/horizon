//! The context a mod's tree part runs in: the engine's [`Run`] behind
//! [`ModTreeContext`](crate::worldgen::mod_tree_parts::ModTreeContext). The part answers plain
//! values, so a failed read or draw is kept and returned when the part is done, and the calls after
//! it do nothing.
use super::{
    FoliageAttachment, Layer, Run, TreeFeature, TreeResult, foliage::FoliagePlacer, read,
    trunk::TrunkPlacer,
};
use crate::registry::BlockStateId;
use crate::worldgen::{
    feature::FeatureWorld,
    feature_graph::FeatureExecutionError,
    mod_tree_parts::{ModFoliageAttachment, ModTreeContext, ModTreeRandom, TreeAxis},
    placement::PlacementContext,
    tree_blocks::{self as tb, Axis, TreeBlocks},
};
use horizon_core::random::RandomSource;

/// A bare random, for the draws that have no world (`foliage_height`).
pub(super) struct Draws<'a, R>(pub &'a mut R);

impl<R: RandomSource> ModTreeRandom for Draws<'_, R> {
    fn next_int_bound(&mut self, bound: i32) -> i32 {
        self.0.next_int_bound(bound).unwrap_or(0)
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

/// Which part of the tree is being placed.
#[derive(Clone, Copy)]
pub(super) enum Phase<'a> {
    Trunk(&'a TrunkPlacer, &'a TreeFeature),
    Foliage(&'a FoliagePlacer, &'a TreeFeature),
    Decoration,
}

pub(super) struct ModCtx<'a, 'w, W: FeatureWorld, R> {
    run: &'a mut Run<'w, W, R>,
    blocks: &'a TreeBlocks,
    phase: Phase<'a>,
    error: Option<FeatureExecutionError<W::Error>>,
}

impl<'a, 'w, W: FeatureWorld + PlacementContext, R: RandomSource> ModCtx<'a, 'w, W, R> {
    pub(super) fn new(
        run: &'a mut Run<'w, W, R>,
        blocks: &'a TreeBlocks,
        phase: Phase<'a>,
    ) -> Self {
        Self {
            run,
            blocks,
            phase,
            error: None,
        }
    }
    /// The first failure the part ran into, if any.
    pub(super) fn finish(self) -> TreeResult<(), W::Error> {
        self.error.map_or(Ok(()), Err)
    }
    /// The value of `result`, or `fallback` once a failure is kept.
    fn keep<T>(&mut self, result: TreeResult<T, W::Error>, fallback: T) -> T {
        if self.error.is_some() {
            return fallback;
        }
        match result {
            Ok(value) => value,
            Err(error) => {
                self.error = Some(error);
                fallback
            }
        }
    }
    fn failed(&self) -> bool {
        self.error.is_some()
    }
}

impl<W: FeatureWorld + PlacementContext, R: RandomSource> ModTreeRandom for ModCtx<'_, '_, W, R> {
    fn next_int_bound(&mut self, bound: i32) -> i32 {
        Draws(&mut *self.run.random).next_int_bound(bound)
    }
    fn next_long(&mut self) -> i64 {
        self.run.random.next_long()
    }
    fn next_float(&mut self) -> f32 {
        self.run.random.next_float()
    }
    fn next_double(&mut self) -> f64 {
        self.run.random.next_double()
    }
    fn next_boolean(&mut self) -> bool {
        self.run.random.next_boolean()
    }
}

impl<W: FeatureWorld + PlacementContext, R: RandomSource> ModTreeContext for ModCtx<'_, '_, W, R> {
    fn state(&self, name: &str) -> Option<BlockStateId> {
        let name = if name.contains(':') {
            name.to_owned()
        } else {
            format!("minecraft:{name}")
        };
        self.blocks.default_state(&name)
    }
    fn with_property(&self, state: BlockStateId, name: &str, value: &str) -> Option<BlockStateId> {
        self.blocks.with(state, name, value)
    }
    fn block(&mut self, pos: [i32; 3]) -> Option<BlockStateId> {
        let result = read(self.run.world, pos).map(Some);
        self.keep(result, None)
    }
    fn is_air(&mut self, pos: [i32; 3]) -> bool {
        self.block(pos).is_some_and(|s| self.blocks.is_air(s))
    }
    fn valid_tree_pos(&mut self, pos: [i32; 3]) -> bool {
        self.block(pos)
            .is_some_and(|s| self.blocks.valid_tree_pos(s))
    }
    fn is_free(&mut self, pos: [i32; 3]) -> bool {
        self.block(pos)
            .is_some_and(|s| self.blocks.valid_tree_pos(s) || self.blocks.has(s, tb::LOGS))
    }
    fn place_log(&mut self, pos: [i32; 3], axis: Option<TreeAxis>) -> bool {
        let Phase::Trunk(placer, tree) = self.phase else {
            return false;
        };
        if self.failed() {
            return false;
        }
        let axis = axis.map(|a| match a {
            TreeAxis::X => Axis::X,
            TreeAxis::Y => Axis::Y,
            TreeAxis::Z => Axis::Z,
        });
        let result = placer.place_log(self.run, tree, pos, axis);
        self.keep(result, false)
    }
    fn place_log_if_free(&mut self, pos: [i32; 3]) {
        let Phase::Trunk(placer, tree) = self.phase else {
            return;
        };
        if self.failed() {
            return;
        }
        let result = placer.place_log_if_free(self.run, tree, pos);
        self.keep(result, ());
    }
    fn place_below_trunk(&mut self, pos: [i32; 3]) {
        let Phase::Trunk(_, tree) = self.phase else {
            return;
        };
        if self.failed() {
            return;
        }
        let result = TrunkPlacer::place_below_trunk(self.run, tree, pos);
        self.keep(result, ());
    }
    fn try_place_leaf(&mut self, pos: [i32; 3]) -> bool {
        let Phase::Foliage(_, tree) = self.phase else {
            return false;
        };
        if self.failed() {
            return false;
        }
        let result = tree.try_place_leaf(self.run, pos);
        self.keep(result, false)
    }
    fn foliage_offset(&mut self) -> i32 {
        let Phase::Foliage(placer, _) = self.phase else {
            return 0;
        };
        if self.failed() {
            return 0;
        }
        let result = placer.sample_offset(self.run.random);
        self.keep(result, 0)
    }
    fn set_decoration(&mut self, pos: [i32; 3], state: BlockStateId) -> bool {
        if !matches!(self.phase, Phase::Decoration) || self.failed() {
            return false;
        }
        let result = self.run.set(Layer::Decoration, pos, state).map(|()| true);
        self.keep(result, false)
    }
}

impl From<ModFoliageAttachment> for FoliageAttachment {
    fn from(a: ModFoliageAttachment) -> Self {
        FoliageAttachment::new(a.pos, a.radius_offset, a.double_trunk)
    }
}
impl From<FoliageAttachment> for ModFoliageAttachment {
    fn from(a: FoliageAttachment) -> Self {
        Self {
            pos: a.pos,
            radius_offset: a.radius_offset,
            double_trunk: a.double_trunk(),
        }
    }
}
