#!/usr/bin/env python3
"""Run the exact game completion expression without linking the whole game."""
import json
import os
from pathlib import Path
import subprocess

root = Path(__file__).resolve().parents[3]
source = (root / "crates/horizon-client/src/game.rs").read_text()
start = source.index("mod_api::__private::resolve_block_keys(")
end = '.map_err(|errors| errors.join("\\n"))?;'
stop = source.index(end, start) + len(end)
fixture = Path(__file__).with_name("block_key_load_boundary.rs").read_text()
fixture = fixture.replace("// PRODUCTION_COMPLETION", source[start:stop])
scratch = root / "target/block-key-load-boundary"
(scratch / "tests").mkdir(parents=True, exist_ok=True)
(scratch / "tests/load.rs").write_text(fixture)
manifest = '[workspace]\n[package]\nname="block-key-load-boundary"\nversion="0.0.0"\nedition="2024"\n[dependencies]\n'
for name in ("horizon-world", "horizon-mod-api"):
    key = "mod_api" if name == "horizon-mod-api" else name.replace("-", "_")
    features = ', features=["testing"]' if name == "horizon-mod-api" else ""
    manifest += f'{key} = {{ package={json.dumps(name)}, path={json.dumps(str(root / "crates" / name))}{features} }}\n'
(scratch / "Cargo.toml").write_text(manifest)
subprocess.run(["cargo", "test", "--manifest-path", str(scratch / "Cargo.toml"), "--test", "load"],
               env=dict(os.environ, CARGO_TARGET_DIR=str(root / "target")), check=True)
