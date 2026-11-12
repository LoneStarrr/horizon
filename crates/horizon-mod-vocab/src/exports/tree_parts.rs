pub use world::tree_parts::{
    FoliageAttachment, FoliagePlacer, FoliagePlacerType, TreeAxis, TreeContext, TreeDecorator,
    TreeDecoratorType, TreeRandom, TreeState, TrunkPlacer, TrunkPlacerType,
};
mod __private {
    /// The tree parts the mods registered, for the executable to hand to the
    /// engine before any world opens.
    pub use crate::world::tree_parts::tree_part_declarations;
}
