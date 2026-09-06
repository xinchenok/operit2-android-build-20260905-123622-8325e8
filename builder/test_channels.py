"""Channel separation and reviewed patches; no Android execution is implied."""
from pathlib import Path
import io
import json
import os
import tempfile
import unittest
from unittest.mock import patch
import zipfile
import channel_policy as policy
import enhanced_source as enhanced
import legacy_packages as legacy
import update_channel as release
from plugin_payload import inspect_toolpkg

class SeparationTests(unittest.TestCase):
    def test_original_never_reads_or_writes_application_files(self):
        with tempfile.TemporaryDirectory() as d,patch.dict(os.environ,{'OPERIT2_CHANNEL':'original'}),patch.object(enhanced,'apply_patches') as modified,patch.object(legacy,'install') as imported:
            p=Path(d)/'source.txt';p.write_bytes(b'original')
            policy.prepare_application(Path(d))
            self.assertEqual(p.read_bytes(),b'original')
            self.assertEqual([p.name for p in Path(d).iterdir()],['source.txt'])
            modified.assert_not_called();imported.assert_not_called()
    def test_invalid_channel_does_not_fall_back_to_enhanced(self):
        with patch.dict(os.environ,{'OPERIT2_CHANNEL':'typo'}),self.assertRaises(ValueError):policy.channel()
    def test_channels_have_different_application_cache_identity(self):
        with patch.dict(os.environ,{'OPERIT2_CHANNEL':'original'}):a=policy.modification_identity()
        with patch.dict(os.environ,{'OPERIT2_CHANNEL':'enhanced','LEGACY_SHA':'a'*40}):b=policy.modification_identity()
        self.assertNotEqual(a,b)
    def test_missing_legacy_checkout_detected_before_changes(self):
        with tempfile.TemporaryDirectory() as d,patch.dict(os.environ,{'OPERIT2_CHANNEL':'enhanced','LEGACY_SOURCE_ROOT':'','LEGACY_SHA':''}),self.assertRaises(RuntimeError):policy.prepare_application(Path(d))

class PatchTests(unittest.TestCase):
    def specs(self):return json.loads(enhanced.CONFIG.read_text())
    def test_patch_transaction_is_repeatable_and_records_original_diff(self):
        with tempfile.TemporaryDirectory() as d:
            root=Path(d);p=root/'example.dart';p.write_text('original')
            config=root/'fixes.json';config.write_text(json.dumps([{'name':'fixture','path':'example.dart','old':'original','new':'fixed'}]))
            with patch.object(enhanced,'CONFIG',config):enhanced.apply_patches(root);enhanced.apply_patches(root)
            self.assertEqual(p.read_text(),'fixed')
            self.assertIn('-original',(root/'tools/release/dist/compatibility/enhanced/application.patch').read_text())
    def test_missing_context_does_not_write_partial_patch(self):
        with tempfile.TemporaryDirectory() as d:
            root=Path(d);p=root/'example.dart';p.write_text('original')
            config=root/'fixes.json';config.write_text(json.dumps([{'name':'first','path':'example.dart','old':'original','new':'fixed'},{'name':'second','path':'example.dart','old':'absent','new':'not-written'}]))
            with patch.object(enhanced,'CONFIG',config),self.assertRaises(RuntimeError):enhanced.apply_patches(root)
            self.assertEqual(p.read_text(),'original')
    def test_regression_contexts_cover_known_failures(self):
        specs=self.specs();text='\n'.join(p['new'] for p in specs)
        for required in ('_chatFlowBindingGeneration > 0','ensureInitialChat','scheduledChatId','_submitInFlight','recordingChatId','submittedReply'):self.assertIn(required,text)
        self.assertEqual(len({p['name'] for p in specs}),len(specs))

class LegacyTests(unittest.TestCase):
    SOURCE='/* METADATA\n{"name":"sample","enabledByDefault":true,"tools":[]}\n*/\nexports.main=()=>Tools.System.shell("x");'
    def test_legacy_is_namespaced_and_not_enabled_by_default(self):
        text,info=legacy.convert(self.SOURCE,'sample.js',set())
        self.assertEqual(info['package_id'],'legacy_sample');self.assertFalse(info['enabled_by_default'])
        self.assertEqual(info['missing_methods'],['System.shell'])
        self.assertIn('throw new Error',text);self.assertIn('Object.create(baseTools)',text)
        self.assertFalse(json.loads(legacy.METADATA.search(text)[1])['enabledByDefault'])
    def test_no_missing_tools_is_not_labeled_runtime_verified(self):
        _,info=legacy.convert(self.SOURCE,'sample.js',{'System.shell'})
        self.assertEqual(info['status'],'static_api_match_only');self.assertFalse(info['device_tested'])
    def test_toolpkg_directory_resource_is_valid(self):
        stream=io.BytesIO()
        with zipfile.ZipFile(stream,'w') as z:
            z.writestr('manifest.json',json.dumps({'main':'main.js','resources':[{'path':'resources/pc'}]}))
            z.writestr('main.js','exports.main=()=>42;');z.writestr('resources/pc/agent.py','print(42)')
        self.assertEqual(inspect_toolpkg(stream.getvalue(),'pc')['entry_check'],'passed')
    def test_empty_resource_still_fails(self):
        stream=io.BytesIO()
        with zipfile.ZipFile(stream,'w') as z:
            z.writestr('manifest.json',json.dumps({'main':'main.js','resources':[{'path':'missing'}]}));z.writestr('main.js','main')
        with self.assertRaises(RuntimeError):inspect_toolpkg(stream.getvalue(),'missing')

class ReleaseTests(unittest.TestCase):
    def test_switching_channels_uses_global_high_water(self):
        rows=[{'tag':'android-enhanced-1000000020-aaaaaaa','channel':'enhanced','version_code':1000000020,'metadata_url':'enh'},{'tag':'android-original-1000000010-bbbbbbb','channel':'original','version_code':1000000010,'metadata_url':'orig'}]
        def meta(url,**kw):
            row=next(x for x in rows if x['metadata_url']==url)
            return {'build_channel':row['channel'],'version_code':row['version_code'],'certificate_sha256':'f'*64}
        with patch.object(release,'published_builds',return_value=rows),patch.object(release,'get',side_effect=meta):last,high=release.release_state('a/b','f'*64,'original')
        self.assertEqual(last['version_code'],1000000010)
        self.assertEqual(release.version_code('version: 2.0+6',{'version_code':high},1),1000000021)
    def test_release_from_other_channel_does_not_satisfy_skip(self):
        row={'tag':'android-enhanced-1000000010-aaaaaaa','channel':'enhanced','version_code':1000000010,'metadata_url':'enh'}
        with patch.object(release,'published_builds',return_value=[row]),patch.object(release,'get',return_value={'build_channel':'enhanced','version_code':1000000010,'certificate_sha256':'f'*64}):last,high=release.release_state('a/b','f'*64,'original')
        self.assertEqual(last,{})
    def test_existing_different_key_is_not_silently_used(self):
        row={'tag':'android-original-1000000010-aaaaaaa','channel':'original','version_code':1000000010,'metadata_url':'orig'}
        with patch.object(release,'published_builds',return_value=[row]),patch.object(release,'get',return_value={'certificate_sha256':'bad'}),self.assertRaises(RuntimeError):release.release_state('a/b','f'*64,'original')
if __name__=='__main__':unittest.main()
