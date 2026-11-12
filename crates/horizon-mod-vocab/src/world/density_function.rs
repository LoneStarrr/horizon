//! Density function types a native mod registers, as
//! [`register_density_function`](crate::Registrar::register_density_function)
//! hands them over once at startup. A data pack's noise router (or any density
//! function) with `"type": "mymod:wave"` then samples the mod's code through
//! the normal noise pipeline, so surface rules, biomes and decoration keep
//! working on the terrain it shapes.
//!
//! There is no `Host` here: density samples on the worldgen worker threads, so
//! a type and its samplers keep no mutable state and answer the same value
//! for the same seed and position.
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

/// What the game knows about a registered type: its namespaced id
/// (`modid:wave`), which a density function's `type` names.
pub(crate) struct DensityFunctionInfo {
    pub(crate) id: Box<str>,
    pub(crate) ty: Arc<dyn DensityFunctionType>,
}

/// The types the mods registered at startup: `modid:wave` → type. Empty before the parts are
/// installed.
// hidden: the game's startup adapter reads the registered types; mods declare them through the registrar.
#[doc(hidden)]
pub fn density_function_declarations() -> Vec<(String, Arc<dyn DensityFunctionType>)> {
    crate::parts::installed_density_function_list()
        .iter()
        .map(|info| (info.id.to_string(), info.ty.clone()))
        .collect()
}
