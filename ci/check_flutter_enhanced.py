"""Type-check actual patched Dart sources and run real widget regressions in CI."""
from pathlib import Path
import subprocess
import sys
wrapper=Path(__file__).resolve().parents[1]
root=Path.cwd()
sys.path.insert(0,str(wrapper/'builder'))
sys.path.insert(0,str(root/'tools/build_scripts'))
from enhanced_source import apply_patches
from common import FLUTTER_APP_DIR, staged_non_ohos_flutter_dependencies, generate_dart_proxy_artifacts
apply_patches(root)
# Rust owns these generated Dart APIs; tests must compile against real outputs.
generate_dart_proxy_artifacts()
with staged_non_ohos_flutter_dependencies():
    subprocess.run(['flutter','pub','get'],cwd=FLUTTER_APP_DIR,check=True)
    subprocess.run(['flutter','test','--no-pub','--reporter','expanded',
                    'test/enhanced_chat_regression_test.dart'],cwd=FLUTTER_APP_DIR,check=True)
