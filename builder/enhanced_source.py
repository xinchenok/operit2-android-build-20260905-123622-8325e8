"""Apply reviewed enhanced-only patches before native/Web compilation."""
from __future__ import annotations
import difflib
import hashlib
import json
from pathlib import Path
import shutil

REVISION = 'chat-initial-scroll-submit-v2'
CONFIG = Path(__file__).resolve().parents[1] / 'enhancements/chat-fixes.json'

def apply_patches(root: Path) -> dict:
    specs = json.loads(CONFIG.read_text(encoding='utf-8'))
    staged = {}
    before = {}
    statuses = []
    for spec in specs:
        path = spec['path']
        if path not in staged:
            before[path] = (root/path).read_text(encoding='utf-8')
            staged[path] = before[path]
        text = staged[path]
        if spec['new'] in text:
            status = 'already_applied'
        elif text.count(spec['old']) == 1:
            staged[path] = text.replace(spec['old'], spec['new'], 1)
            status = 'applied'
        else:
            raise RuntimeError(f"Enhanced patch '{spec['name']}' needs review against updated upstream {path}. Original channel is unaffected.")
        statuses.append({'name':spec['name'], 'status':status})
    output = root/'tools/release/dist/compatibility/enhanced'
    output.mkdir(parents=True, exist_ok=True)
    patch_path = output/'application.patch'
    differences = []
    for path, text in staged.items():
        if text != before[path]:
            differences.append(''.join(difflib.unified_diff(before[path].splitlines(True),text.splitlines(True),fromfile='a/'+path,tofile='b/'+path)))
            (root/path).write_text(text,encoding='utf-8',newline='\n')
    if differences:
        patch_path.write_text(''.join(differences),encoding='utf-8')
    result = {'revision':REVISION,'patches':statuses,'files':{p:hashlib.sha256(t.encode()).hexdigest() for p,t in staged.items()},'device_tested':False}
    (output/'manifest.json').write_text(json.dumps(result,indent=2)+'\n',encoding='utf-8')
    tests = CONFIG.parent/'enhanced_chat_regression_test.dart'
    if tests.exists():
        fixture = (root/'apps/flutter/app/test/chat_area_streaming_cursor_test.dart').read_text(encoding='utf-8')
        fixture = fixture.replace('void main() {', 'void upstreamRegressionTests() {', 1)
        fixture = fixture.replace("import 'dart:async';", "import 'dart:async';\nimport 'package:operit2/ui/features/chat/screens/AIChatScreen.dart';", 1)
        fixture = fixture.replace('  bool isLoading = true,', '  ValueChanged<bool>? onFollowChanged,\n  bool isLoading = true,', 1)
        fixture = fixture.replace('onAutoScrollToBottomChanged: (_) {},', 'onAutoScrollToBottomChanged: onFollowChanged ?? (_) {},', 1)
        # Build a metrics snapshot from the mounted controller; do not depend on
        # an unexported SDK implementation class in the copied upstream test.
        fixture = fixture.replace('final metrics = FixedScrollMetrics(',
                                  'final metrics = scrollController.position.copyWith(')
        extra = tests.read_text(encoding='utf-8')
        # The generated API uses an enum, not a string, for structured parts.
        extra = extra.replace("kind: 'markdown'", 'kind: MessagePartKind.markdown')
        (root/'apps/flutter/app/test/enhanced_chat_regression_test.dart').write_text(
            fixture + '\n' + extra, encoding='utf-8')
    return result


def run_flutter_regressions(root: Path) -> None:
    """Execute the copied regression suite with the real generated proxy API."""
    from common import (FLUTTER_APP_DIR, flutter_command, flutter_pub_get,
                        run, staged_non_ohos_flutter_dependencies)
    with staged_non_ohos_flutter_dependencies():
        flutter_pub_get()
        run([flutter_command(), 'test', '--no-pub', '--reporter', 'expanded',
             'test/enhanced_chat_regression_test.dart'], cwd=FLUTTER_APP_DIR)
