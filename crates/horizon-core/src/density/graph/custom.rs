//! Density function types registered from outside the engine. The graph keeps
//! a decoded instance per distinct (type, config) pair and compiles it, with
//! the world seed, into an ordinary sampler.
use super::*;

/// One decoded use of a custom density type: the type's config, validated.
pub trait CustomDensity: std::fmt::Debug + Send + Sync {
    /// The smallest and largest value the function can return (vanilla's
    /// `minValue`/`maxValue`), which the interval pass trusts.
    fn range(&self) -> [f32; 2];
    /// The axes the value depends on, as vanilla's bit mask (x = 1, y = 2,
    /// z = 4). Defaults to all three, the conservative answer.
    fn domain_axes(&self) -> u8 {
        7
    }
    /// The sampler for a world with `seed`. Samplers are shared by every
    /// worker, so the same seed must give the same values.
    fn compile(&self, seed: i64) -> Arc<dyn DensitySampler>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CustomId(pub(super) usize);

#[derive(Clone, Debug)]
pub(super) struct CustomEntry {
    pub(super) kind: String,
    pub(super) config: String,
    pub(super) instance: Arc<dyn CustomDensity>,
}

impl DensityGraph {
    /// Register a decoded custom function of type `kind` with its canonical
    /// `config` text. The same type and config give the same id, so equal
    /// uses share one node.
    pub fn register_custom(
        &mut self,
        kind: String,
        config: String,
        instance: Arc<dyn CustomDensity>,
    ) -> CustomId {
        if let Some(index) = self
            .customs
            .iter()
            .position(|c| c.kind == kind && c.config == config)
        {
            return CustomId(index);
        }
        self.customs.push(CustomEntry {
            kind,
            config,
            instance,
        });
        CustomId(self.customs.len() - 1)
    }
    pub(super) fn custom_entry(&self, id: CustomId) -> Result<&CustomEntry, CompileError> {
        self.customs.get(id.0).ok_or(CompileError::UnknownCustom)
    }
}
