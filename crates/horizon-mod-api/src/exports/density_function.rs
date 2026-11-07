mod density_function;
pub use density_function::{CustomDensity, DensityFunctionType, DensitySampler};
mod __private {
    /// The density function types the mods registered, for the executable to
    /// hand to the engine before any world opens.
    pub use crate::density_function::installed_density_functions;
}
