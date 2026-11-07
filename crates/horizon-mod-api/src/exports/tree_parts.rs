mod tree_parts;
pub use tree_parts::{
    FoliageAttachment, FoliagePlacer, FoliagePlacerType, TreeAxis, TreeContext, TreeDecorator,
    TreeDecoratorType, TreeRandom, TreeState, TrunkPlacer, TrunkPlacerType,
};
mod __private {
    /// The tree parts the mods registered, for the executable to hand to the
    /// engine before any world opens.
    pub use crate::tree_parts::installed_tree_parts;
}
