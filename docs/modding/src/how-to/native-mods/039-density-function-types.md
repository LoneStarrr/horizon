<!-- chapter: Density function types -->
<!-- separator: 0 -->

### Density function types

A density function type is a mod's code where a data pack names one: any density function in
a pack, a noise router's `final_density` or a `"type": "minecraft:add"` argument, whose `"type"` is
`"mymod:wave"` is sampled by the mod's `DensityFunctionType` (vanilla's `DensityFunction` codec,
and `sample_value` its `compute`). Register it once, at startup, in `Hooks::register`; the id is
in the mod's namespace:

```rust,ignore
struct Wave;

impl DensityFunctionType for Wave {
    fn decode(&self, config: &serde_json::Value) -> Result<Arc<dyn CustomDensity>, String> {
        let period = config["period"].as_f64().ok_or("`period` must be a number")?;
        if period <= 0.0 {
            return Err("`period` must be positive".into());
        }
        Ok(Arc::new(WaveUse { period }))
    }
}

#[derive(Debug)]
struct WaveUse {
    period: f64,
}

impl CustomDensity for WaveUse {
    fn range(&self) -> [f32; 2] {
        [-1.0, 1.0]
    }
    fn compile(&self, seed: i64) -> Arc<dyn DensitySampler> {
        let mut random = XoroshiroRandom::new(seed);
        let noise = NormalNoise::create_parity(-3, &[1.0, 0.5])
            .and_then(|parameters| parameters.create(&mut random.fork_positional().from_hash_of("mymod:wave")))
            .expect("fixed parameters are valid");
        Arc::new(WaveSampler { period: self.period, noise })
    }
}

#[derive(Debug)]
struct WaveSampler {
    period: f64,
    noise: NoiseStack,
}

impl DensitySampler for WaveSampler {
    fn sample_value(&self, [x, y, z]: [i32; 3]) -> f32 {
        let wave = (f64::from(x) / self.period).sin() as f32;
        (wave + self.noise.get3(f64::from(x), f64::from(y), f64::from(z))).clamp(-1.0, 1.0)
    }
}

#[mod_api::horizon_mod]
impl Hooks for MyMod {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        r.register_density_function("wave", Wave);
    }
}
```

A pack then uses it like any vanilla function, in a noise settings file's router or in
`data/mymod/worldgen/density_function/`:

```json
{ "type": "minecraft:add", "argument1": { "type": "mymod:wave", "period": 24.0 }, "argument2": "minecraft:overworld/base_3d_noise" }
```

#### Instead of a chunk generator

[`register_chunk_generator`](#chunk-generators) replaces the terrain step and gives the level
nothing else. A density function type changes only the shape the noise pipeline reads, so the
level keeps what vanilla builds around a noise generator: its surface rules, biomes, carvers,
aquifers and features. Use a density type when the terrain is a formula over `(x, y, z)` that
the data pack's router can combine with vanilla functions; use a chunk generator when the mod
places the blocks itself.

#### The contract

`DensityFunctionType::decode(&self, config)` gets the function's JSON object, `type` field
included, and answers a `CustomDensity` for that one use, or an `Err` message saying what is wrong
(the load error names the type and your message). Validate everything here: `compile` cannot fail.
Two uses with equal configs share one node of the density graph.

`CustomDensity` has three methods:

- `range` declares the smallest and largest value the function can return, vanilla's `minValue`
  and `maxValue`. See below.
- `domain_axes` says which axes the value depends on, as vanilla's bit mask (x = 1, y = 2, z = 4).
  It defaults to 7, all three, which is always safe; a function of `x` and `z` only can answer 5.
- `compile(seed)` builds the `DensitySampler` of one world from its seed.

`DensitySampler::sample_value(position: [i32; 3]) -> f32` answers the value at a block position,
and is called very often (every cell corner of every chunk), so it should only compute.

#### The declared range

The engine does not sample every block to know where a level has ground, air or neither. Its
interval pass bounds each function from the `range`s of its parts, to skip the sky and the deep
stone, and trusts what it is told. A sampler that returns a value outside its `range` makes the
bounds wrong, and terrain the engine skipped as empty or solid can differ from what the sampler
would have said. Keep the range honest, and clamp in `sample_value` if the formula can
overshoot, as `Wave` does. A range that is too wide is safe and only slower. `range` must not be NaN.

#### Seeding

The same seed gives the same values, in every world and on every worker. Seed from the
`seed` of `compile`, never from time or a global. The API wraps vanilla's random and noise, so
a sampler makes the numbers the vanilla functions do:

- `XoroshiroRandom::new(seed)` is `XoroshiroRandomSource(seed)`, with `next_long`, `next_double`
  and `fork_positional`.
- `PositionalRandom::from_hash_of(name)` is a random for a name, such as a noise's id, so each
  noise of a mod has its own stream; `at([x, y, z])` is the random of a block position.
- `NormalNoise::create_parity(first_octave, amplitudes)` builds the parameters of a noise as a
  noise JSON does, with vanilla's normalization, and `create(&mut random)` seeds them into a
  `NoiseStack`. `NoiseStack::range` is the stack's own range, for a `range` that follows it.
  `PerlinNoise::new(&mut random)` is a single `ImprovedNoise` octave. Both give `get2` and `get3`.

These are the API's own types: a mod never sees the engine's.

#### Threads and caching

There is no `Host` in density sampling. Worldgen workers call the same sampler at once, so a
type, its uses and its samplers are `Send + Sync`, keep no mutable state and have no interior
mutability that changes an answer. Build everything that costs something, such as a noise stack,
in `compile`, which gives one sampler per seed that every worker shares; do not rebuild it in
`sample_value`. Cache per seed by building in `compile`, not by filling a cache lazily: a cache
filled by whichever worker asks first makes answers depend on the order.

#### Refusals

A density function type in another mod's namespace, in the `minecraft` namespace, an empty name,
a name outside vanilla's resource location rules (lowercase letters, digits and `_ . - /`) and an
id registered twice stop the game at startup with a message naming the mod. A pack that names a
type nobody registered, say after its mod was removed, fails to load with `unknown density type:
mymod:wave`, and a type's own `decode` error fails it with your message.

#### Not on wasm

A density function type is native only: it is code the worldgen workers run, and
`register_density_function` is native only. A mod built to `mod.wasm` that calls it fails when it
registers, with a message saying why. A data pack's JSON can still name a type a native mod
registered.
