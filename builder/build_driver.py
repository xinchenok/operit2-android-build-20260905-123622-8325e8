"""CI-only orchestration: inherited tools, real prerequisites, reusable Web bundle.

No application code or signing policy is changed. Run from the pinned source root.
This module's local unit tests do not replace a Windows/Android integration build.
"""
from __future__ import annotations

import argparse
import json
import os
import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path.cwd()
sys.path.insert(0, str(ROOT / "tools" / "build_scripts"))


def prepend_path(directory: Path) -> None:
    """Update the parent environment, not merely one subprocess's env argument."""
    value = str(directory.resolve())
    parts = [p for p in os.environ.get("PATH", "").split(os.pathsep) if p]
    normalized = os.path.normcase(value)
    parts = [p for p in parts if os.path.normcase(os.path.abspath(p)) != normalized]
    os.environ["PATH"] = os.pathsep.join([value, *parts])


def prepare_tools() -> Path:
    from common import ensure_typescript

    version = os.environ.get("TYPESCRIPT_VERSION", "5.9.3")
    binary_dir = ensure_typescript(version)
    prepend_path(binary_dir)
    command = "tsc.cmd" if os.name == "nt" else "tsc"
    resolved = shutil.which(command)
    expected = (binary_dir / command).resolve()
    if resolved is None or Path(resolved).resolve() != expected:
        raise RuntimeError(f"TypeScript did not resolve to the CI installation: {resolved}")
    output = subprocess.check_output([resolved, "--version"], text=True).strip()
    if output != f"Version {version}":
        raise RuntimeError(f"Unexpected TypeScript version: {output}")

    # Test the same inherited lookup used inside Gradle -> Python -> tsc.cmd.
    python = ROOT / ".venv" / ("Scripts/python.exe" if os.name == "nt" else "bin/python")
    subprocess.run(
        [str(python), "-c",
         "import subprocess,sys; subprocess.run([sys.argv[1], '--version'], check=True)",
         command], check=True, cwd=ROOT,
    )
    # Also expose this directory to subsequent, independent GitHub Actions steps.
    if os.environ.get("GITHUB_PATH"):
        with open(os.environ["GITHUB_PATH"], "a", encoding="utf-8", newline="\n") as file:
            file.write(str(binary_dir.resolve()) + "\n")
    print(f"Verified inherited TypeScript lookup: {resolved}", flush=True)
    return binary_dir


def preflight() -> None:
    from common import (
        ANDROID_LOCAL_PROPERTIES, DIST_DIR, FLUTTER_APP_DIR,
        flutter_command, flutter_pub_get, generate_dart_proxy_artifacts,
        read_properties, run, staged_non_ohos_flutter_dependencies, write_properties,
    )
    from build_flutter_android import configure_android_flutter_sdk, ensure_android_signing
    from prepare_android import patch_dynamic_color, prepare_gradle_wrapper

    prepare_tools()
    ensure_android_signing()
    flutter = flutter_command()
    configure_android_flutter_sdk(flutter)
    sdk = os.environ.get("ANDROID_HOME") or os.environ.get("ANDROID_SDK_ROOT")
    if sdk:
        properties = read_properties(ANDROID_LOCAL_PROPERTIES)
        properties["sdk.dir"] = Path(sdk).as_posix()
        write_properties(ANDROID_LOCAL_PROPERTIES, properties)
    android = FLUTTER_APP_DIR / "android"
    gradle = prepare_gradle_wrapper(flutter, android)
    with staged_non_ohos_flutter_dependencies():
        flutter_pub_get()
        flutter_pub_get(enforce_lockfile=True)
        patch_dynamic_color(FLUTTER_APP_DIR, DIST_DIR)
        # Unlike `help`, these tasks execute the previously failing plugin packer.
        run([str(gradle), ":app:syncOperitPlugins", ":app:verifyOperitAndroidRuntimeArtifacts",
             "--no-daemon", "--console=plain", "--stacktrace",
             "-Ptarget-platform=android-arm64"], cwd=android)
        generate_dart_proxy_artifacts()
    print("Actual Gradle plugin packing, runtime payload checks and Dart code generation passed.", flush=True)


def web_identity(source_commit: str) -> dict[str, str | int]:
    return {
        "schema": 1,
        "source_commit": source_commit,
        "flutter": "3.41.9",
        "rust": os.environ.get("RUST_TOOLCHAIN_VERSION", "1.95.0"),
        "typescript": os.environ.get("TYPESCRIPT_VERSION", "5.9.3"),
        "terser": os.environ.get("TERSER_VERSION", "5.44.0"),
        "wasm_bindgen": os.environ.get("WASM_BINDGEN_VERSION", "0.2.122"),
        "wasi_sdk": os.environ.get("WASI_SDK_VERSION", "20.0"),
        "base_href": "/",
        "runtime_plugins": "sync-runtime-before-web-v1",
    }


def valid_web_cache(bundle: Path, marker: Path, identity: dict) -> bool:
    from common import (
        WEB_ACCESS_REQUIRED_FILES, WEB_ACCESS_VERSION_FILE,
        compute_web_access_bundle_digest, read_web_access_version_manifest,
    )

    try:
        if json.loads(marker.read_text(encoding="utf-8")) != identity:
            return False
        if any(not (bundle / name).is_file() for name in WEB_ACCESS_REQUIRED_FILES):
            return False
        manifest = read_web_access_version_manifest(bundle / WEB_ACCESS_VERSION_FILE)
        if not manifest:
            return False
        digest, count, size = compute_web_access_bundle_digest(bundle)
        return (digest, count, size) == (
            manifest["contentHash"], manifest["fileCount"], manifest["byteSize"],
        )
    except (OSError, ValueError, KeyError, TypeError, RuntimeError) as error:
        print(f"Web checkpoint is missing or unusable; rebuilding: {error}", flush=True)
        return False


def build_web() -> None:
    from common import WEB_ACCESS_BUNDLE_DIR, prepare_web_access_embedded_assets
    from build_flutter_web_access import main as upstream_web_build

    prepare_tools()
    commit = subprocess.check_output(["git", "rev-parse", "HEAD"], text=True).strip()
    identity = web_identity(commit)
    marker = WEB_ACCESS_BUNDLE_DIR.parent / "ci-provenance.json"
    if valid_web_cache(WEB_ACCESS_BUNDLE_DIR, marker, identity):
        # preflight() generates the Dart proxies even when the Web build is reused.
        print("Reusing the verified Web bundle for this exact source and toolchain.", flush=True)
        prepare_web_access_embedded_assets()
    else:
        marker.unlink(missing_ok=True)
        result = upstream_web_build("/")
        if result != 0:
            raise RuntimeError(f"Web build failed: {result}")
        marker.write_text(json.dumps(identity, sort_keys=True, indent=2) + "\n", encoding="utf-8")
        if not valid_web_cache(WEB_ACCESS_BUNDLE_DIR, marker, identity):
            marker.unlink(missing_ok=True)
            raise RuntimeError("Newly built Web bundle failed its content manifest check")
    print("Web checkpoint complete; it can be saved before APK compilation.", flush=True)


def build_apk() -> None:
    from build_arm64 import build
    from common import DIST_DIR

    prepare_tools()
    # Updating os.environ ensures the entire Flutter -> Gradle -> Python tree inherits PATH.
    build()
    shutil.copy2(__file__, DIST_DIR / "build_driver.py")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("stage", choices=("preflight", "web", "apk"))
    args = parser.parse_args()
    {"preflight": preflight, "web": build_web, "apk": build_apk}[args.stage]()


if __name__ == "__main__":
    main()
