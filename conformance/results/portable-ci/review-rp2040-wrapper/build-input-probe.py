from pathlib import Path
import sys,tempfile,shutil
from unittest.mock import patch
sys.path.insert(0,str(Path(__file__).parent/'final/tools/emulators/rp2040js'))
import firmware
base=Path(__file__).parent
for stable in [True,False]:
 with tempfile.TemporaryDirectory(dir=base) as tmp:
  root=Path(tmp); out=root/'bundle';out.mkdir();(out/'build.json').write_text('old')
  elf=root/'conformance/targets/rp2040/target/thumbv6m-none-eabi/release/nepl3-conformance-rp2040';elf.parent.mkdir(parents=True);shutil.copyfile(base/'input-bundle/firmware.elf',elf)
  pairs=[{'src':'before'},{'src':'before' if stable else 'after'}]
  with patch.object(firmware,'ROOT',root),patch.object(firmware,'source_inputs',side_effect=pairs),patch.object(firmware,'command',return_value='fixture-tool'):
   if stable:firmware.build(out);assert (out/'build.json').exists()
   else:
    try:firmware.build(out)
    except ValueError as error:assert 'changed' in str(error)
    else:raise AssertionError('changed source accepted')
    assert not (out/'build.json').exists()
 print('mocked build input stability',stable,'pass')
