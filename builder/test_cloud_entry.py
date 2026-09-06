"""Regression checks for the GitHub-only build entry; no Android compilation."""
from pathlib import Path
import os
import re
import tempfile
import unittest
from unittest.mock import patch
import update_channel

ROOT = Path(__file__).resolve().parent.parent


class CloudEntryTests(unittest.TestCase):
    def test_web_build_needs_no_custom_input(self):
        text = (ROOT / '.github/workflows/update-android.yml').read_text(encoding='utf-8')
        header = text.split('jobs:', 1)[0]
        self.assertIn('name: Build latest Android', header)
        self.assertIn('workflow_dispatch:', header)
        self.assertNotIn('inputs:', header)
        self.assertNotIn('request_id', text)
        self.assertNotIn('Update-Operit2.cmd', text)
        self.assertNotIn('Start-Build.cmd', text)

    def test_runtime_and_apk_share_the_resolved_latest_commit(self):
        text = (ROOT / '.github/workflows/update-android.yml').read_text(encoding='utf-8')
        refs = re.findall(r'repository: AAswordman/Operit2\n\s+ref: ([^\n]+)', text)
        self.assertEqual(refs, ['${{ needs.resolve.outputs.source_sha }}'] * 2)
        self.assertEqual(text.count('${{ secrets.OPERIT2_UPDATE_SIGNING }}'), 2)
        self.assertNotIn('prepare_signing.ps1', text)

    def test_missing_key_directs_to_github_not_a_desktop_script(self):
        with tempfile.TemporaryDirectory() as tmp:
            summary = Path(tmp) / 'summary.md'
            env = {'OPERIT2_UPDATE_SIGNING': '', 'GITHUB_STEP_SUMMARY': str(summary),
                   'GITHUB_REPOSITORY': 'example/build'}
            with patch.dict(os.environ, env), patch.object(update_channel, 'api') as network:
                with self.assertRaisesRegex(RuntimeError, 'Fixed signing is not configured') as error:
                    update_channel.resolve()
                network.assert_not_called()
            self.assertIn('GitHub Settings', str(error.exception))
            self.assertNotIn('.cmd', str(error.exception))
            self.assertIn('https://github.com/example/build/settings/secrets/actions/new', summary.read_text(encoding='utf-8'))

    def test_legacy_entry_cannot_build_old_apks(self):
        text = (ROOT / '.github/workflows/operit2-android-test.yml').read_text(encoding='utf-8')
        self.assertNotIn('push:', text)
        self.assertNotIn('build_arm64.py', text)
        self.assertNotIn('303192ccf12a8873d9d391228be026096de95db5', text)
        self.assertIn('actions/workflows/update-android.yml', text)


if __name__ == '__main__':
    unittest.main()
