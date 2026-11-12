use super::*;
/// The density function types registered at startup, for the executable to hand to the engine.
pub(crate) fn installed_density_function_list()
-> &'static [crate::world::density_function::DensityFunctionInfo] {
    installed().map_or(&[], |installed| installed.density_functions.as_slice())
}

impl dyn Registrar + '_ {
    /// Register density function type `name` (`wave` or `mymod:wave`) once at
    /// startup: a density function in a data pack with `"type": "mymod:wave"`
    /// then samples by `ty`, which decodes its JSON, declares its value range
    /// and builds a sampler from the world's seed. The sampler runs on the
    /// worldgen worker threads. The id is in the calling mod's namespace;
    /// another mod's namespace, the `minecraft` namespace, an empty name, a name outside
    /// those rules and an
    /// id registered twice refuse the mods at startup, naming the mod. The name
    /// follows vanilla's resource location rules: lowercase letters, digits and
    /// `_ . - /`.
    pub fn register_density_function<T: crate::DensityFunctionType>(&mut self, name: &str, ty: T) {
        self.add_density_function(name, Arc::new(ty));
    }
}

/// Registrar operations for density function types.
pub trait DensityRegistrar {
    /// Register density function type `name` with `ty`; mods reach it through
    /// `register_density_function`.
    ///
    /// Not on wasm: density samplers are native code that runs on the worldgen worker threads,
    /// which a wasm mod's cannot hand over.
    fn add_density_function(&mut self, name: &str, ty: Arc<dyn crate::DensityFunctionType>) {
        let _ = ty;
        panic!("density function {name:?}: a wasm mod cannot register a density function type");
    }
}
