from __future__ import annotations
import io
import json
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest
from unittest.mock import patch
from types import SimpleNamespace
import zipfile
import plugin_payload as payload
ORIGINAL_TIMER = '''        const timeoutPromise = new Promise<null>((resolve) => {
            setTimeout(() => resolve(null), timeoutMs);
        });

        const sendResult = await Promise.race([sendPromise, timeoutPromise]);'''

def toolpkg(main: bool = True) -> bytes:
    stream = io.BytesIO()
    with zipfile.ZipFile(stream, 'w', zipfile.ZIP_DEFLATED) as archive:
        archive.writestr('manifest.json', json.dumps({'toolpkg_id': 'fixture', 'main': 'dist/main.js'}))
        if main:archive.writestr('dist/main.js', 'exports.main = () => 42;')
    return stream.getvalue()

class SourceFixTests(unittest.TestCase):
    def test_typescript_fix_cleans_both_outcomes(self):
        text,status=payload.fix_source(ORIGINAL_TIMER)
        self.assertEqual(status['timer_cleanup'],'applied')
        self.assertIn('ReturnType<typeof setTimeout>',text)
        self.assertIn('.finally(() => {',text)
        self.assertIn('clearTimeout(timeoutId)',text)
    def test_preserves_windows_line_endings(self):
        text,_=payload.fix_source(ORIGINAL_TIMER.replace('\n','\r\n'))
        self.assertNotIn('\n',text.replace('\r\n',''))
    def test_repeat_does_not_apply_twice(self):
        once,_=payload.fix_source(ORIGINAL_TIMER+'\n'+payload.OLD_ERROR)
        twice,status=payload.fix_source(once)
        self.assertEqual(once,twice)
        self.assertEqual(set(status.values()),{'already_applied'})
    def test_unknown_upstream_not_overwritten(self):
        source='const result = await revisedUpstreamImplementation();'
        text,status=payload.fix_source(source)
        self.assertEqual(text,source)
        self.assertEqual(set(status.values()),{'upstream_changed_not_modified'})
    def test_error_message_identifies_actual_tool(self):
        text,_=payload.fix_source(payload.OLD_ERROR+'\n'+payload.OLD_ERROR)
        self.assertNotIn('读取对话消息失败',text)
        self.assertEqual(text.count("func.name.replace(/_impl$/, '')"),2)
    def test_repeated_stages_preserve_original_patch(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory);source=root/payload.PLUGIN_SOURCE
            source.parent.mkdir(parents=True);source.write_text(ORIGINAL_TIMER,encoding='utf-8')
            first=payload.apply_source_fixes(root);second=payload.apply_source_fixes(root)
            self.assertEqual(first,second)
            backup=root/'tools/release/dist/compatibility/extended_chat/extended_chat.ts.before'
            self.assertEqual(backup.read_text(encoding='utf-8'),ORIGINAL_TIMER)
    @unittest.skipUnless(shutil.which('node'),'Node is needed for timer behavior regression')
    def test_real_javascript_resolve_reject_and_timeout(self):
        fixed,_=payload.fix_source(ORIGINAL_TIMER.replace('<null>',''))
        function='async function run(sendPromise, timeoutMs, setTimeout, clearTimeout) {\n'+fixed+'\nreturn sendResult;\n}'
        javascript=function+r'''
const assert = require('node:assert/strict');
(async () => {
  for (const outcome of ['resolved', 'rejected', 'timeout']) {
    const timers = new Map(); let next = 0;
    const set = (cb) => { const id = next++; timers.set(id, cb); return id; };
    const clear = (id) => timers.delete(id);
    let resolveSend, rejectSend;
    const send = new Promise((resolve, reject) => { resolveSend = resolve; rejectSend = reject; });
    const result = run(send, 180000, set, clear);
    assert.equal(timers.size, 1);
    if (outcome === 'resolved') { resolveSend('answer'); assert.equal(await result, 'answer'); }
    if (outcome === 'rejected') { rejectSend(new Error('network')); await assert.rejects(result, /network/); }
    if (outcome === 'timeout') { timers.values().next().value(); assert.equal(await result, null); resolveSend('late'); }
    assert.equal(timers.size, 0, outcome + ' must release timer (including timer ID zero)');
  }
})().catch(e => { console.error(e); process.exitCode = 1; });
'''
        result=subprocess.run(['node','-e',javascript],capture_output=True,text=True)
        self.assertEqual(result.returncode,0,result.stderr)

class CacheTests(unittest.TestCase):
    def test_original_has_no_source_fix_identity(self):
        import os
        import build_driver
        with patch.dict(os.environ,{'OPERIT2_CHANNEL':'original'}):
            self.assertEqual(build_driver.web_identity('a'*40)['application_identity'],{'channel':'original','application_modifications':False})
    def test_enhanced_identity_contains_actual_source_patch_digest(self):
        import os
        import build_driver
        with patch.dict(os.environ,{'OPERIT2_CHANNEL':'enhanced','LEGACY_SHA':'a'*40}):
            first=build_driver.web_identity('b'*40)
            self.assertTrue(first['application_identity']['patch_digest'])
            with patch.dict(os.environ,{'LEGACY_SHA':'c'*40}):self.assertNotEqual(first,build_driver.web_identity('b'*40))

class PayloadTests(unittest.TestCase):
    def test_archive_entrypoint_present(self):
        self.assertEqual(payload.inspect_toolpkg(toolpkg(),'fixture')['entry_check'],'passed')
    def test_archive_missing_compiled_entry_is_rejected(self):
        with self.assertRaisesRegex(RuntimeError,'Missing/empty ToolPkg entry'):payload.inspect_toolpkg(toolpkg(False),'fixture')
    def test_archive_without_manifest_is_rejected(self):
        stream=io.BytesIO()
        with zipfile.ZipFile(stream,'w') as archive:archive.writestr('dist/main.js','exports.main = () => 1')
        with self.assertRaisesRegex(RuntimeError,'Missing ToolPkg manifest'):payload.inspect_toolpkg(stream.getvalue(),'fixture')
    def test_hjson_is_not_misparsed_as_json(self):
        stream=io.BytesIO()
        with zipfile.ZipFile(stream,'w') as archive:archive.writestr('manifest.hjson','{main: dist/main.js}')
        self.assertEqual(payload.inspect_toolpkg(stream.getvalue(),'fixture')['entry_check'],'upstream_hjson_loader')
    def _check_bytes(self,native:bytes):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory);output=root/'fixture.js'
            output.write_bytes(b'/* METADATA {"name":"fixture"} */\nexports.main = () => 42;')
            with patch.object(payload,'generated_plugins',return_value=[('buildin','fixture.js',output)]):
                return payload.inspect_generated(root,native)
    def test_filename_alone_cannot_pass_native_verification(self):
        with self.assertRaisesRegex(RuntimeError,'lacks the current plugin bytes'):self._check_bytes(b'\x7fELF fixture.js')
    def test_stale_native_payload_is_rejected(self):
        with self.assertRaisesRegex(RuntimeError,'lacks the current plugin bytes'):self._check_bytes(b'/* METADATA {"name":"fixture"} */\nexports.main = () => 41;')
    def test_complete_native_bytes_pass(self):
        content=b'/* METADATA {"name":"fixture"} */\nexports.main = () => 42;'
        report=self._check_bytes(b'ELF-prefix-'+content+b'-suffix')
        self.assertTrue(report['native_bytes_verified']);self.assertEqual(report['packages'][0]['native_offset'],11)
    def test_missing_generated_file_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory)
            for group in ('buildin','external'):(root/'plugins/packages'/group).mkdir(parents=True)
            sync=SimpleNamespace(_collect_sync_plan=lambda _:[SimpleNamespace(destination_name='missing.js')])
            with patch.object(payload,'load_sync',return_value=sync),self.assertRaisesRegex(RuntimeError,'Plugin sync produced no usable output'):
                payload.generated_plugins(root)
    def test_generated_empty_builtin_list_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory)
            for group in ('buildin','external'):(root/'plugins/packages'/group).mkdir(parents=True)
            with patch.object(payload,'load_sync',return_value=SimpleNamespace(_collect_sync_plan=lambda _:[])),self.assertRaisesRegex(RuntimeError,'No built-in packages'):
                payload.generated_plugins(root)
if __name__=='__main__':unittest.main()
