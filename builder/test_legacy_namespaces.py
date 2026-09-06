"""Imported package ids must still resolve from bundled UI and IPC code."""
import io
import json
from pathlib import Path
import subprocess
import tempfile
from types import SimpleNamespace
import unittest
import zipfile

import legacy_packages as legacy


class LegacyNamespaceTests(unittest.TestCase):
    def test_syntax_check_accepts_commonjs_and_bundled_browser_modules(self):
        legacy.check_javascript(b'exports.open = () => true;', 'dist/main.js')
        legacy.check_javascript(b'export function createCommandsPage() {}', 'resources/commands-page.js')

    def test_syntax_error_reports_the_archive_path_and_parser_error(self):
        with self.assertRaisesRegex(RuntimeError, r'Invalid bundled JavaScript resources/broken.js:') as error:
            legacy.check_javascript(b'export function broken( {', 'resources/broken.js')
        self.assertIn('SyntaxError', str(error.exception))

    def test_known_package_references_move_but_paths_and_labels_do_not(self):
        source = '''const LINUX_SSH_PACKAGE_NAME = "linux_ssh";
exports.QQBOT_TOOLPKG_ID = "com.operit.qqbot_bundle";
exports.SUBPACKAGE_ID = "plan_mode_tools";
const info = {packageName: "apk_reverse"};
const DEFAULT_HIDDEN_EXECUTOR_NAME = "linux_ssh";
const label = "linux_ssh";
const resource = "dist/packages/linux_ssh.js";
const unrelated = {packageName: "android.system"};'''
        mapping = {'linux_ssh': 'legacy_linux_ssh',
                   'com.operit.qqbot_bundle': 'legacy.com.operit.qqbot_bundle',
                   'plan_mode_tools': 'legacy_plan_mode_tools',
                   'apk_reverse': 'legacy_apk_reverse'}
        result = legacy.namespace_references(source, mapping)
        for expected in ('LINUX_SSH_PACKAGE_NAME = "legacy_linux_ssh"',
                         'QQBOT_TOOLPKG_ID = "legacy.com.operit.qqbot_bundle"',
                         'SUBPACKAGE_ID = "legacy_plan_mode_tools"',
                         'packageName: "legacy_apk_reverse"'):
            self.assertIn(expected, result)
        for unchanged in source.splitlines()[4:]:
            self.assertIn(unchanged, result)

    def test_archive_ui_can_resolve_the_renamed_subpackage(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            manifest = {'toolpkg_id': 'com.operit.linux_ssh_bundle',
                        'main': 'main.js',
                        'subpackages': [{'id': 'linux_ssh', 'entry': 'package.js'}]}
            (root/'manifest.json').write_text(json.dumps(manifest), encoding='utf-8')
            (root/'main.js').write_text('exports.registerToolPkg = () => {};', encoding='utf-8')
            (root/'package.js').write_text(
                '/* METADATA {"name":"linux_ssh","tools":[]} */\nexports.test=()=>true;',
                encoding='utf-8')
            (root/'ui.js').write_text(
                'const LINUX_SSH_PACKAGE_NAME = "linux_ssh";\n'
                'exports.open = ctx => ctx.importPackage(LINUX_SSH_PACKAGE_NAME);',
                encoding='utf-8')
            sync = SimpleNamespace(_iter_files_for_pack=lambda _legacy, folder: sorted(folder.iterdir()))
            data, info = legacy._convert_toolpkg(root, set(), sync, root)
            with zipfile.ZipFile(io.BytesIO(data)) as archive:
                packaged = json.loads(archive.read('manifest.json'))
                renamed = packaged['subpackages'][0]['id']
                ui = archive.read('ui.js').decode()
                script = ui + '\nexports.open({importPackage(id) {if (id !== ' + json.dumps(renamed) + ') throw Error(id);}});'
                subprocess.run(['node', '-'], input=script, text=True, check=True, capture_output=True)
                metadata = legacy.METADATA.search(archive.read('package.js').decode())[1]
                self.assertEqual(json.loads(metadata)['name'], renamed)
            self.assertEqual(info['package_id'], 'legacy.com.operit.linux_ssh_bundle')

    def test_unported_old_host_classes_block_packaging(self):
        source = ('/* METADATA {"name":"old","tools":[]} */\n'
                  'const Service = Java.type("com.ai.assistance.operit.api.chat.EnhancedAIService");')
        with self.assertRaisesRegex(RuntimeError, 'unported Operit1 classes'):
            legacy.convert(source, 'old.js', set())

    def test_android_java_classes_are_not_misidentified_as_old_operit_classes(self):
        info = legacy.host_dependencies('const File = Java.type("java.io.File"); Android.getContext();')
        self.assertEqual(info['native_global_dependencies'], ['Android', 'Java'])
        self.assertEqual(info['legacy_host_class_dependencies'], [])


if __name__ == '__main__':
    unittest.main()
