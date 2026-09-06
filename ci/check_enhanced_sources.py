"""Use real upstream source checkouts to verify patch contexts and all legacy packages."""
from pathlib import Path
import json
import os
import subprocess
import sys
sys.path.insert(0, str(Path(__file__).resolve().parents[1]/'builder'))
from channel_policy import prepare_application
from plugin_payload import inspect_toolpkg

root=Path(sys.argv[1]).resolve();legacy=Path(sys.argv[2]).resolve()
sha=subprocess.check_output(['git','-C',str(legacy),'rev-parse','HEAD'],text=True).strip()
os.environ.update(OPERIT2_CHANNEL='original')
before=subprocess.check_output(['git','-C',str(root),'diff','--binary','HEAD'])
prepare_application(root)
assert subprocess.check_output(['git','-C',str(root),'diff','--binary','HEAD'])==before
os.environ.update(OPERIT2_CHANNEL='enhanced',LEGACY_SOURCE_ROOT=str(legacy),LEGACY_SHA=sha)
first=prepare_application(root)
second=prepare_application(root)
assert first['legacy_packages']['packages']==second['legacy_packages']['packages']
for file in (root/'plugins/packages/buildin').glob('legacy_*.toolpkg'):
    inspect_toolpkg(file.read_bytes(),file.name)
print(json.dumps({'source_commit':subprocess.check_output(['git','-C',str(root),'rev-parse','HEAD'],text=True).strip(),
 'legacy_commit':sha,'packages':first['legacy_packages']['count'],
 'patch_contexts':len(first['chat_fixes']['patches']),'repeatable':True},indent=2))
