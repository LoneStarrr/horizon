mod feature_type;
pub use feature_type::{
    ConfiguredFeature, FeatureHeightmap, FeatureRandom, FeatureRegion, FeatureState, FeatureType,
};
mod __private {
    /// The feature types the mods registered, for the executable to hand to
    /// the engine before any world opens.
    pub use crate::feature_type::installed_feature_types;
}
