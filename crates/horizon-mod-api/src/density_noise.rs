//! The random and noise a density sampler seeds itself with: vanilla's
//! `XoroshiroRandom`, its positional factory, `NormalNoise` and
//! `ImprovedNoise`, as types of the API's own. Each wraps the engine's type,
//! which the mod never sees, so the engine can change without breaking a mod.
use horizon_core::noise::Noise as _;
use horizon_core::noise::{
    NoiseStack as EngineStack, NormalNoise as EngineNormal, PerlinNoise as EnginePerlin,
};
use horizon_core::random::{
    PositionalRandom as _, RandomSource as _, XoroshiroPositionalRandom,
    XoroshiroRandom as EngineRandom,
};

/// Why a noise could not be built.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct NoiseError(pub String);

impl std::fmt::Display for NoiseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for NoiseError {}

/// Vanilla's `XoroshiroRandom` (Xoroshiro128++).
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct XoroshiroRandom(EngineRandom);

impl XoroshiroRandom {
    /// `XoroshiroRandomSource(seed)`.
    pub fn new(seed: i64) -> Self {
        Self(EngineRandom::new(seed))
    }
    /// `nextLong`.
    pub fn next_long(&mut self) -> i64 {
        self.0.next_long()
    }
    /// `nextDouble`.
    pub fn next_double(&mut self) -> f64 {
        self.0.next_double()
    }
    /// `forkPositional`.
    pub fn fork_positional(&mut self) -> PositionalRandom {
        PositionalRandom(self.0.fork_positional())
    }
}

/// Vanilla's `XoroshiroRandomSource.XoroshiroPositionalRandomFactory`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct PositionalRandom(XoroshiroPositionalRandom);

impl PositionalRandom {
    /// `fromHashOf`: the random of a name, such as a noise's id.
    pub fn from_hash_of(&self, name: &str) -> XoroshiroRandom {
        XoroshiroRandom(self.0.from_hash_of(name))
    }
    /// `at`: the random of a block position.
    pub fn at(&self, pos: [i32; 3]) -> XoroshiroRandom {
        XoroshiroRandom(self.0.at(crate::BlockPos::new(pos[0], pos[1], pos[2])))
    }
}

/// Vanilla's `ImprovedNoise`: one octave of Perlin noise.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct PerlinNoise(EnginePerlin);

impl PerlinNoise {
    /// `new ImprovedNoise(random)`.
    pub fn new(random: &mut XoroshiroRandom) -> Self {
        Self(EnginePerlin::new(&mut random.0))
    }
    /// `noise(x, 0, z)`.
    pub fn get2(&self, x: f64, z: f64) -> f32 {
        self.0.get2(x, z)
    }
    /// `noise(x, y, z)`.
    pub fn get3(&self, x: f64, y: f64, z: f64) -> f32 {
        self.0.get3(x, y, z)
    }
}

/// A `NormalNoise` seeded from a random: vanilla's `NormalNoise.create`.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct NoiseStack(EngineStack);

impl NoiseStack {
    /// The smallest and largest value it can return, or `None` when its parameters give a NaN range.
    pub fn range(&self) -> Option<[f32; 2]> {
        self.0.range().ok()
    }
    /// `getValue(x, 0, z)`.
    pub fn get2(&self, x: f64, z: f64) -> f32 {
        self.0.get2(x, z)
    }
    /// `getValue(x, y, z)`.
    pub fn get3(&self, x: f64, y: f64, z: f64) -> f32 {
        self.0.get3(x, y, z)
    }
}

/// Vanilla's `NormalNoise.NoiseParameters`: the octaves and amplitudes of a noise, before it is seeded.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct NormalNoise(EngineNormal);

impl NormalNoise {
    /// The parameters `firstOctave` and `amplitudes` of a noise JSON, with vanilla's normalization.
    pub fn create_parity(first_octave: i32, amplitudes: &[f64]) -> Result<Self, NoiseError> {
        EngineNormal::create_parity(first_octave, amplitudes)
            .map(Self)
            .map_err(|error| NoiseError(format!("{error:?}")))
    }
    /// `NormalNoise.create(random, parameters)`.
    pub fn create(&self, random: &mut XoroshiroRandom) -> Result<NoiseStack, NoiseError> {
        self.0
            .create(&mut random.0)
            .map(NoiseStack)
            .map_err(|error| NoiseError(format!("{error:?}")))
    }
}
