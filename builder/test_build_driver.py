"""Orchestration regression tests; these do not compile Flutter or Android."""
import contextlib
import hashlib
import json
import os
import subprocess
import sys
import tempfile
import types
import unittest
import venv
from pathlib import Path
from unittest.mock import patch

import build_driver as driver


def digest_bundle(bundle):
    digest = hashlib.sha256()
    count = size = 0
    for item in sorted(p for p in bundle.rglob("*") if p.is_file()):
        name = item.relative_to(bundle).as_posix()
        if name == "web_access_version.json":
            continue
        data = item.read_bytes()
        digest.update(name.encode())
        digest.update(b"\0")
        digest.update(len(data).to_bytes(8, "big"))
        digest.update(data)
        count += 1
        size += len(data)
    return digest.hexdigest(), count, size


class CheckpointTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.bundle = self.root / "bundle"
        self.bundle.mkdir()
        self.marker = self.root / "ci-provenance.json"
        self.identity = driver.web_identity("source-a")
        (self.bundle / "index.html").write_text("web-output")
        digest, count, size = digest_bundle(self.bundle)
        (self.bundle / "web_access_version.json").write_text(json.dumps({
            "contentHash": digest, "fileCount": count, "byteSize": size,
        }))
        self.marker.write_text(json.dumps(self.identity))
        self.common = types.SimpleNamespace(
            WEB_ACCESS_REQUIRED_FILES=("index.html", "web_access_version.json"),
            WEB_ACCESS_VERSION_FILE="web_access_version.json",
            compute_web_access_bundle_digest=digest_bundle,
            read_web_access_version_manifest=lambda p: json.loads(p.read_text()),
        )
        patched = patch.dict(sys.modules, {"common": self.common})
        patched.start()
        self.addCleanup(patched.stop)

    def valid(self):
        return driver.valid_web_cache(self.bundle, self.marker, self.identity)

    def test_verified_bundle_reused(self):
        self.assertTrue(self.valid())

    def test_source_change_requires_build(self):
        self.identity = driver.web_identity("source-b")
        self.assertFalse(self.valid())

    def test_toolchain_change_requires_build(self):
        self.identity["typescript"] = "different-version"
        self.assertFalse(self.valid())

    def test_content_change_requires_build(self):
        (self.bundle / "index.html").write_text("changed")
        self.assertFalse(self.valid())

    def test_missing_file_requires_build(self):
        (self.bundle / "index.html").unlink()
        self.assertFalse(self.valid())

    def test_incomplete_provenance_requires_build(self):
        self.marker.unlink()
        self.assertFalse(self.valid())

    def test_corrupt_manifest_requires_build(self):
        (self.bundle / "web_access_version.json").write_text("{bad")
        self.assertFalse(self.valid())

    def test_extra_file_requires_build(self):
        (self.bundle / "extra.js").write_text("extra")
        self.assertFalse(self.valid())

    def test_corrupt_provenance_does_not_delete_outputs(self):
        self.marker.write_text("not-json")
        self.assertFalse(self.valid())
        self.assertTrue((self.bundle / "index.html").exists())


class ToolPathTests(unittest.TestCase):
    def test_path_prepend_is_idempotent(self):
        with tempfile.TemporaryDirectory() as directory:
            with patch.dict(os.environ, {"PATH": os.defpath}):
                driver.prepend_path(Path(directory))
                first = os.environ["PATH"]
                driver.prepend_path(Path(directory))
                self.assertEqual(first, os.environ["PATH"])
                self.assertEqual(first.split(os.pathsep)[0], str(Path(directory).resolve()))

    def test_real_nested_process_inherits_tool_and_persists_path(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            venv.EnvBuilder(with_pip=False).create(root / ".venv")
            binaries = root / "tools"
            binaries.mkdir()
            executable = binaries / ("tsc.cmd" if os.name == "nt" else "tsc")
            executable.write_text("@echo Version 5.9.3\n" if os.name == "nt"
                                  else "#!/bin/sh\necho 'Version 5.9.3'\n")
            executable.chmod(0o755)
            export = root / "github-path"
            common = types.SimpleNamespace(ensure_typescript=lambda version: binaries)
            with patch.object(driver, "ROOT", root), patch.dict(sys.modules, {"common": common}), \
                    patch.dict(os.environ, {"TYPESCRIPT_VERSION": "5.9.3", "GITHUB_PATH": str(export)}):
                self.assertEqual(driver.prepare_tools(), binaries)
                self.assertEqual(export.read_text().strip(), str(binaries.resolve()))

    def test_mismatched_tool_version_is_not_used(self):
        common = types.SimpleNamespace(ensure_typescript=lambda version: Path("tools"))
        with patch.dict(sys.modules, {"common": common}), \
                patch.object(driver.shutil, "which", return_value=str((Path("tools") / ("tsc.cmd" if os.name == "nt" else "tsc")).resolve())), \
                patch.object(driver.subprocess, "check_output", return_value="Version WRONG"), \
                patch.dict(os.environ, {"TYPESCRIPT_VERSION": "5.9.3"}):
            with self.assertRaisesRegex(RuntimeError, "Unexpected TypeScript"):
                driver.prepare_tools()


class PreflightTests(unittest.TestCase):
    def test_runs_real_gradle_tasks_before_web_not_just_help(self):
        commands = []
        calls = []
        common = types.SimpleNamespace(
            ANDROID_LOCAL_PROPERTIES=Path("android/local.properties"), DIST_DIR=Path("dist"),
            FLUTTER_APP_DIR=Path("app"), flutter_command=lambda: "flutter",
            flutter_pub_get=lambda **kw: calls.append("pub"),
            generate_dart_proxy_artifacts=lambda: calls.append("proxy"),
            read_properties=lambda p: {}, write_properties=lambda *args: None,
            run=lambda command, **kw: commands.append(command),
            staged_non_ohos_flutter_dependencies=contextlib.nullcontext,
        )
        android = types.SimpleNamespace(configure_android_flutter_sdk=lambda f: None,
                                        ensure_android_signing=lambda: None)
        prepare = types.SimpleNamespace(patch_dynamic_color=lambda *args: calls.append("patch"),
                                         prepare_gradle_wrapper=lambda *args: Path("gradlew"))
        with patch.dict(sys.modules, {"common": common, "build_flutter_android": android,
                                       "prepare_android": prepare}), \
                patch.object(driver, "prepare_tools", side_effect=lambda: calls.append("tools")):
            driver.preflight()
        self.assertEqual(calls[0], "tools")
        self.assertEqual(len(commands), 1)
        self.assertIn(":app:syncOperitPlugins", commands[0])
        self.assertIn(":app:verifyOperitAndroidRuntimeArtifacts", commands[0])
        self.assertNotIn("help", commands[0])
        self.assertIn("proxy", calls)


if __name__ == "__main__":
    unittest.main(verbosity=2)
