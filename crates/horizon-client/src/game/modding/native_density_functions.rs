//! Native density function types: the mod's types behind the engine's density graph.
use horizon_core::density::graph::CustomDensity as EngineCustom;
use horizon_core::density::{DensitySampler as EngineSampler, SampleError, SamplerContext};
use horizon_world::worldgen::mod_density::{ModDensityType, ModDensityTypes};
use mod_api::{CustomDensity, DensityFunctionType, DensitySampler};
use std::sync::Arc;

/// A mod's type as the engine's: what it decodes reaches the engine as its own
/// custom density, the mod's sampler behind the engine's sampler trait.
struct Adapter(Arc<dyn DensityFunctionType>);

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

/// The types the mods registered at startup, as the engine reads them:
/// `modid:wave` -> type. Empty before the parts are installed.
pub(in crate::game) fn installed_density_functions() -> ModDensityTypes {
    mod_api::__private::density_function_declarations()
        .into_iter()
        .map(|(id, ty)| (id, Arc::new(Adapter(ty)) as Arc<dyn ModDensityType>))
        .collect()
}

#[cfg(test)]
#[path = "native_density_functions/tests.rs"]
mod tests;
