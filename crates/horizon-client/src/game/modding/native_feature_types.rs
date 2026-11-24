//! Native feature types: the mod's types behind the engine's feature placement.
use horizon_world::heightmap::HeightmapType as EngineHeightmap;
use horizon_world::registry::BlockStateId;
use horizon_world::worldgen::mod_features::{
    ModFeature, ModFeatureRandom, ModFeatureType, ModFeatureTypes, ModFeatureWorld,
    install_mod_feature_types,
};
use mod_api::{
    ConfiguredFeature, FeatureHeightmap, FeatureRandom, FeatureRegion, FeatureState, FeatureType,
};
use std::sync::Arc;

/// A mod's type as the engine's: what it decodes reaches the engine as its own
/// feature, the engine's region and random behind the mod's traits.
struct Adapter(Arc<dyn FeatureType>);

impl ModFeatureType for Adapter {
    fn decode(&self, config: &serde_json::Value) -> Result<Arc<dyn ModFeature>, String> {
        Ok(Arc::new(EngineFeature(self.0.decode(config)?)))
    }
}

#[derive(Debug)]
struct EngineFeature(Arc<dyn ConfiguredFeature>);

impl ModFeature for EngineFeature {
    fn place(
        &self,
        world: &mut dyn ModFeatureWorld,
        random: &mut dyn ModFeatureRandom,
        origin: [i32; 3],
    ) -> bool {
        self.0
            .place(&mut Converting(world), &mut Draws(random), origin)
    }
}

/// The engine's region, as the mod's [`FeatureRegion`].
struct Converting<'a>(&'a mut dyn ModFeatureWorld);

impl FeatureRegion for Converting<'_> {
    fn min_y(&mut self) -> i32 {
        self.0.min_y()
    }
    fn max_y(&mut self) -> i32 {
        self.0.max_y()
    }
    fn block(&mut self, pos: [i32; 3]) -> Option<FeatureState> {
        self.0.block(pos).map(|s| FeatureState::from_raw(s.0))
    }
    fn set_block(&mut self, pos: [i32; 3], state: FeatureState, flags: i32) -> bool {
        self.0.set_block(pos, BlockStateId(state.raw()), flags)
    }
    fn height(&mut self, kind: FeatureHeightmap, x: i32, z: i32) -> Option<i32> {
        let kind = match kind {
            FeatureHeightmap::WorldSurfaceWg => EngineHeightmap::WorldSurfaceWg,
            FeatureHeightmap::WorldSurface => EngineHeightmap::WorldSurface,
            FeatureHeightmap::OceanFloorWg => EngineHeightmap::OceanFloorWg,
            FeatureHeightmap::OceanFloor => EngineHeightmap::OceanFloor,
            FeatureHeightmap::MotionBlocking => EngineHeightmap::MotionBlocking,
            FeatureHeightmap::MotionBlockingNoLeaves => EngineHeightmap::MotionBlockingNoLeaves,
            _ => return None,
        };
        self.0.height(kind, x, z)
    }
    fn state(&mut self, name: &str, properties: &[(&str, &str)]) -> Option<FeatureState> {
        self.0
            .state(name, properties)
            .map(|s| FeatureState::from_raw(s.0))
    }
    fn is_air(&mut self, state: FeatureState) -> bool {
        self.0.is_air(BlockStateId(state.raw()))
    }
    fn block_in_tag(&mut self, pos: [i32; 3], tag: &str) -> bool {
        self.0.block_in_tag(pos, tag)
    }
}

/// The engine's feature random, as the mod's [`FeatureRandom`].
struct Draws<'a>(&'a mut dyn ModFeatureRandom);

impl FeatureRandom for Draws<'_> {
    fn next_int_bound(&mut self, bound: i32) -> i32 {
        self.0.next_int_bound(bound)
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

/// The types the mods registered at startup, as the engine reads them:
/// `modid:spikes` -> type. Empty before the parts are installed.
pub(in crate::game) fn installed_feature_types() -> ModFeatureTypes {
    mod_api::__private::feature_type_declarations()
        .into_iter()
        .map(|(id, ty)| (id, Arc::new(Adapter(ty)) as Arc<dyn ModFeatureType>))
        .collect()
}

/// Hand the feature types the mods registered to the engine, before any world opens.
pub(in crate::game) fn install() {
    if install_mod_feature_types(installed_feature_types()).is_err() {
        log::warn!("Mod feature types already installed; keeping the first table");
    }
}

#[cfg(test)]
#[path = "native_feature_types/tests.rs"]
mod tests;
