"""Type-check actual patched Dart sources and run real widget regressions in CI."""
from pathlib import Path
import subprocess
import sys

wrapper = Path(__file__).resolve().parents[1]
root = Path.cwd()
sys.path.insert(0, str(wrapper / 'builder'))
sys.path.insert(0, str(root / 'tools/build_scripts'))

from enhanced_source import apply_patches
from build_driver import prepare_tools, build_web
from common import (
    FLUTTER_APP_DIR,
    staged_non_ohos_flutter_dependencies,
    generate_dart_proxy_artifacts,
)

apply_patches(root)
# Flutter's native build hook invokes the project's Python and TypeScript tools.
# Each Actions job has its own PATH; the inspect job's tsc is not inherited here.
subprocess.run([sys.executable, '-m', 'venv', str(root / '.venv')], check=True)
prepare_tools()
print('Tested upstream commit:', subprocess.check_output(
    ['git', 'rev-parse', 'HEAD'], text=True).strip(), flush=True)
# Generate the actual Core APIs, never substitute stub generated source files.
generate_dart_proxy_artifacts()
with staged_non_ohos_flutter_dependencies():
    subprocess.run(['flutter', 'pub', 'get'], cwd=FLUTTER_APP_DIR, check=True)
    subprocess.run([
        'dart', 'analyze', '--fatal-infos',
        'test/enhanced_chat_regression_test.dart',
    ], cwd=FLUTTER_APP_DIR, check=True)
# Type errors should fail before spending time on the real Web prerequisite.
# The unmodified native Flutter hook needs its complete content manifest.
build_web()
with staged_non_ohos_flutter_dependencies():
    subprocess.run(['flutter', 'pub', 'get'], cwd=FLUTTER_APP_DIR, check=True)
    subprocess.run([
        'flutter', 'test', '--no-pub', '--reporter', 'expanded',
        'test/enhanced_chat_regression_test.dart',
    ], cwd=FLUTTER_APP_DIR, check=True)
