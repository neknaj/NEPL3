from pathlib import Path
import shutil
root=Path(__file__).resolve().parent;src=root/'source';probe=root/'probe';(probe/'tests').mkdir(parents=True,exist_ok=True)
manifest='[package]\nname="independent-map-union"\nversion="0.0.0"\nedition="2024"\n[workspace]\n[dependencies]\n'
for name in ['core','reader','wire']:
    manifest+=f'nepl3-{name}={{path="../source/crates/foundation/{name}"}}\n'
(probe/'Cargo.toml').write_text(manifest,encoding='utf-8')
body=(src/'crates/foundation/reader/tests/runtime.rs').read_bytes()
(probe/'tests/union.rs').write_bytes(body+b'\nmod independent;\n')
shutil.copytree(src/'crates/foundation/reader/tests/runtime',probe/'tests/runtime',dirs_exist_ok=True)
(probe/'tests/independent.rs').write_bytes((root/'independent.rs').read_bytes())
