//! Density function types a native mod registers, as
//! [`register_density_function`](super::Registrar::register_density_function)
//! hands them over once at startup. A data pack's noise router (or any density
//! function) with `"type": "mymod:wave"` then samples the mod's code through
//! the normal noise pipeline, so surface rules, biomes and decoration keep
//! working on the terrain it shapes.
//!
//! There is no `Host` here: density samples on the worldgen worker threads, so
//! a type and its samplers keep no mutable state and answer the same value
//! for the same seed and position.
use horizon_core::density::graph::CustomDensity as EngineCustom;
use horizon_core::density::{DensitySampler as EngineSampler, SampleError, SamplerContext};
use horizon_world::worldgen::mod_density::{ModDensityType, ModDensityTypes};
use std::sync::Arc;

/// A mod's density function type: vanilla's `DensityFunction` codec for one
/// `modid:wave` id. Implemented once, kept by the game.
pub trait DensityFunctionType: Send + Sync + 'static {
    /// Decode one use of the type from its JSON object, `type` field
    /// included, or say why it is invalid (the message names the type in the
    /// load error). The result declares the function's value
    /// [`range`](CustomDensity::range), which the interval pass trusts, and
    /// builds a sampler from the world's seed.
    fn decode(&self, config: &serde_json::Value) -> Result<Arc<dyn CustomDensity>, String>;
}

/// One decoded use of a density function type, its config validated.
pub trait CustomDensity: std::fmt::Debug + Send + Sync {
    /// The smallest and largest value the function can return (vanilla's
    /// `minValue`/`maxValue`), which the interval pass trusts.
    fn range(&self) -> [f32; 2];
    /// The axes the value depends on, as vanilla's bit mask (x = 1, y = 2,
    /// z = 4). Defaults to all three, the conservative answer.
    fn domain_axes(&self) -> u8 {
        7
    }
    /// The sampler for a world with `seed`. Every worldgen worker shares it, so
    /// the same seed must give the same values.
    fn compile(&self, seed: i64) -> Arc<dyn DensitySampler>;
}

/// A density function of one world: vanilla's `DensityFunction.compute`.
pub trait DensitySampler: std::fmt::Debug + Send + Sync {
    /// The value at the block `position` (`[x, y, z]`).
    fn sample_value(&self, position: [i32; 3]) -> f32;
}

/// A mod's type as the engine's: what it decodes reaches the engine as its own
/// custom density, the mod's sampler behind the engine's sampler trait.
pub(crate) struct Adapter(pub(crate) Arc<dyn DensityFunctionType>);

impl ModDensityType for Adapter {
    fn decode(&self, config: &serde_json::Value) -> Result<Arc<dyn EngineCustom>, String> {
        let custom = self.0.decode(config)?;
        Ok(Arc::new(EngineDensity(custom)))
    }
}

#[derive(Debug)]
struct EngineDensity(Arc<dyn CustomDensity>);

impl EngineCustom for EngineDensity {
    fn range(&self) -> [f32; 2] {
        self.0.range()
    }
    fn domain_axes(&self) -> u8 {
        self.0.domain_axes()
    }
    fn compile(&self, seed: i64) -> Arc<dyn EngineSampler> {
        Arc::new(EngineSamplerOf(self.0.compile(seed)))
    }
}

#[derive(Debug)]
struct EngineSamplerOf(Arc<dyn DensitySampler>);

impl EngineSampler for EngineSamplerOf {
    fn sample_value(&self, _: &mut SamplerContext, position: [i32; 3]) -> Result<f32, SampleError> {
        Ok(self.0.sample_value(position))
    }
}

/// What the game knows about a registered type: its namespaced id
/// (`modid:wave`), which a density function's `type` names, and its engine side.
pub(crate) struct DensityFunctionInfo {
    pub(crate) id: Box<str>,
    pub(crate) ty: Arc<dyn ModDensityType>,
}

/// The types the mods registered at startup, as the engine reads them:
/// `modid:wave` → type. Empty before the parts are installed.
pub fn installed_density_functions() -> ModDensityTypes {
    super::parts::installed_density_function_list()
        .iter()
        .map(|info| (info.id.to_string(), info.ty.clone()))
        .collect()
}
