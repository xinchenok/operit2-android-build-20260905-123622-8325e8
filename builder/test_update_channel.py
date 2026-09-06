import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
import update_channel as c
import prepare_android as p

class VersionTests(unittest.TestCase):
    def test_tool_pin(self):
        t=c.tool_versions('{"flutter":"3.44.1"}', '    RUST_TOOLCHAIN_VERSION: 1.97.0\n    FVM_VERSION: "4.3.1"\n    WASI_SDK_VERSION: "20.0"\n')
        self.assertEqual(t['FLUTTER_VERSION'],'3.44.1');self.assertEqual(t['RUST_TOOLCHAIN_VERSION'],'1.97.0')
        self.assertEqual(t['FVM_VERSION'],'4.3.1')
    def test_reject_moving_flutter(self):
        with self.assertRaises(RuntimeError):c.tool_versions('{"flutter":"stable"}', '')
    def test_build_numbers_increase(self):
        self.assertEqual(c.version_code('version: 2.0.0+12\n',{},5),1000000005)
        self.assertEqual(c.version_code('version: 2.0.0+12\n',{'version_code':1000000999},6),1000001000)
    def test_upstream_higher_build(self):
        self.assertEqual(c.version_code('version: 3.0.0+1500000000\n',{},6),1500000000)
    def test_invalid_version(self):
        with self.assertRaises(RuntimeError):c.version_code('version: 1.0.0',{},1)
    def test_build_limit(self):
        with self.assertRaises(RuntimeError):c.version_code('version: 1.0+2100000001',{},1)

class PatchTests(unittest.TestCase):
    def setUp(self):
        self.tmp=tempfile.TemporaryDirectory();self.addCleanup(self.tmp.cleanup)
        self.root=Path(self.tmp.name);self.app=self.root/'app';self.pkg=self.root/'pkg';self.dist=self.root/'dist'
        self.app.mkdir();(self.pkg/'android').mkdir(parents=True)
        (self.pkg/'pubspec.yaml').write_text('version: 1.9.0\n')
        (self.pkg/'LICENSE').write_text('test license')
        (self.app/'.flutter-plugins-dependencies').write_text(json.dumps({'plugins':{'android':[{'name':'dynamic_color','path':str(self.pkg)}]}}))
    def test_legacy_backport_idempotent(self):
        f=self.pkg/'android/build.gradle.kts';f.write_text(p.OLD_BLOCK)
        p.patch_dynamic_color(self.app,self.dist);p.patch_dynamic_color(self.app,self.dist)
        self.assertEqual(f.read_text(),p.NEW_BLOCK)
    def test_new_version_unchanged(self):
        (self.pkg/'pubspec.yaml').write_text('version: 2.1.0\n')
        f=self.pkg/'android/build.gradle.kts';f.write_text('new upstream script')
        p.patch_dynamic_color(self.app,self.dist)
        self.assertEqual(f.read_text(),'new upstream script')
        self.assertTrue((self.dist/'compatibility/dynamic_color-status.json').exists())
    def test_absent_plugin_recorded(self):
        (self.app/'.flutter-plugins-dependencies').write_text('{"plugins":{"android":[]}}')
        p.patch_dynamic_color(self.app,self.dist)
        self.assertTrue((self.dist/'compatibility/dynamic_color-status.json').exists())
    def test_changed_upstream_not_overwritten(self):
        f=self.pkg/'android/build.gradle.kts';f.write_text('changed script')
        p.patch_dynamic_color(self.app,self.dist)
        self.assertEqual(f.read_text(),'changed script')

class ResolveTests(unittest.TestCase):
    def setUp(self):
        self.env=patch.dict(os.environ,{'OPERIT2_CHANNEL':'original','LEGACY_SHA':''})
        self.env.start();self.addCleanup(self.env.stop)
    def test_missing_key_fails_before_network(self):
        with patch.dict(os.environ,{'OPERIT2_UPDATE_SIGNING':''}),patch.object(c,'api') as request:
            with self.assertRaises(RuntimeError):c.resolve()
            request.assert_not_called()
    def test_malformed_key_fails_before_network(self):
        with patch.dict(os.environ,{'OPERIT2_UPDATE_SIGNING':'not-json'}),patch.object(c,'api') as request:
            with self.assertRaises(RuntimeError):c.resolve()
            request.assert_not_called()
    def _simulate(self, source='a'*40, last=None, certificate='b'*64):
        t=tempfile.TemporaryDirectory();self.addCleanup(t.cleanup);root=Path(t.name)
        env={'OPERIT2_UPDATE_SIGNING':json.dumps({'schema':1,'password':'test','pfx_base64':'test','certificate_sha256':certificate}),
             'GITHUB_REPOSITORY':'owner/build','GITHUB_SHA':'c'*40,'GITHUB_RUN_NUMBER':'3',
             'GITHUB_OUTPUT':str(root/'out'),'GITHUB_STEP_SUMMARY':str(root/'summary')}
        def answer(path, **kw):
            if path==f'repos/{c.UPSTREAM}':return {'default_branch':'main'}
            if '/commits/' in path:return {'sha':source}
            if '/releases?' in path:return [] if last is None else [{'tag_name':'android-original-'+str(last.get('version_code',1000000002))+'-'+('a'*7),'assets':[{'name':'UPDATE.json','browser_download_url':'https://example.invalid/meta'}]}]
            if '/contents/tools/android-runtime' in path:return [{'path':'tools/android-runtime/test','sha':'d'*40}]
            raise AssertionError(path)
        with patch.dict(os.environ,env),patch.object(c,'api',side_effect=answer),patch.object(c,'get',return_value=last),patch.object(c,'source_file',side_effect=lambda path,sha: '{"flutter":"3.41.9"}' if path.endswith('.fvmrc') else ('version: 1.0.0+1\n' if path.endswith('pubspec.yaml') else '')):
            c.resolve()
        return dict(line.split('=',1) for line in (root/'out').read_text().splitlines())
    def test_new_source_builds(self):self.assertEqual(self._simulate()['should_build'],'true')
    def test_same_commit_skips_expensive_build(self):
        last={'source_commit':'a'*40,'builder_commit':'c'*40,'certificate_sha256':'b'*64,'version_code':1000000002,'build_channel':'original','application_identity':{'channel':'original','application_modifications':False}}
        self.assertEqual(self._simulate(last=last)['should_build'],'false')
    def test_new_builder_rebuilds(self):
        last={'source_commit':'a'*40,'builder_commit':'e'*40,'certificate_sha256':'b'*64,'version_code':1000000002,'build_channel':'original','application_identity':{'channel':'original','application_modifications':False}}
        self.assertEqual(self._simulate(last=last)['should_build'],'true')
    def test_key_change_is_not_silent(self):
        with self.assertRaises(RuntimeError):self._simulate(last={'certificate_sha256':'f'*64})

class SignatureTests(unittest.TestCase):
    def test_old_signer_output(self):
        self.assertEqual(c.certificate_digest('Signer #1 certificate SHA-256 digest: '+ 'a'*64),'a'*64)
    def test_new_signer_output(self):
        self.assertEqual(c.certificate_digest('V2 Signer: certificate SHA-256 digest: '+ 'a'*64),'a'*64)
    def test_inconsistent_signers(self):
        with self.assertRaises(RuntimeError):c.certificate_digest('Signer #1 certificate SHA-256 digest: '+ 'a'*64 +'\nSigner #2 certificate SHA-256 digest: '+ 'b'*64)
    def test_no_signer(self):
        with self.assertRaises(RuntimeError):c.certificate_digest('no certificate')
if __name__=='__main__':unittest.main()
