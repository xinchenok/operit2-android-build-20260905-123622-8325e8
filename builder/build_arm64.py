"""Build upstream Operit2 for ARM64 using its existing Android build helpers."""
from __future__ import annotations
import json
import os
import shutil
import subprocess
import sys
from pathlib import Path
from prepare_android import patch_dynamic_color
from channel_policy import channel

# FlutterPluginConstants.ARCH_ARM64 = 2; split outputs add ABI_VERSION * 1000.
ARM64_SPLIT_VERSION_OFFSET = 2_000

def build() -> None:
    root = Path.cwd()
    sys.path.insert(0, str(root / "tools" / "build_scripts"))
    from common import (
        DIST_DIR, FLUTTER_APP_DIR, copy_required_file, flutter_command,
        flutter_pub_get, prepare_web_access_embedded_assets,
        run, staged_non_ohos_flutter_dependencies,
    )
    from build_flutter_android import configure_android_flutter_sdk, ensure_android_signing
    prepare_web_access_embedded_assets()
    ensure_android_signing()
    flutter = flutter_command()
    configure_android_flutter_sdk(flutter)
    with staged_non_ohos_flutter_dependencies():
        flutter_pub_get()
        flutter_pub_get(enforce_lockfile=True)
        compatibility = patch_dynamic_color(FLUTTER_APP_DIR, DIST_DIR)
        DIST_DIR.mkdir(parents=True, exist_ok=True)
        shutil.copy2(FLUTTER_APP_DIR / "pubspec.lock", DIST_DIR / "resolved-pubspec.lock")
        shutil.copy2(FLUTTER_APP_DIR / "pubspec.yaml", DIST_DIR / "resolved-pubspec.yaml")
        command = [flutter, "build", "apk", "--release", "--no-pub",
                   "--target-platform", "android-arm64", "--split-per-abi"]
        build_number = os.environ.get("OPERIT2_BUILD_NUMBER")
        if build_number:
            if not build_number.isdecimal() or not 0 < int(build_number) <= 2100000000:
                raise RuntimeError("Invalid Android build number")
            # The update manifest records the final APK version, not Flutter's
            # base version. Keep ABI splitting to exclude other native ABIs.
            flutter_build_number = int(build_number) - ARM64_SPLIT_VERSION_OFFSET
            if flutter_build_number <= 0:
                raise RuntimeError("Android update version must exceed the ARM64 split offset")
            print(f"ARM64 split: Flutter base {flutter_build_number}, final versionCode {build_number}", flush=True)
            command.extend(["--build-number", str(flutter_build_number)])
        run(command, cwd=FLUTTER_APP_DIR)
    apk = FLUTTER_APP_DIR / "build/app/outputs/flutter-apk/app-arm64-v8a-release.apk"
    copy_required_file(apk, DIST_DIR / "operit2-android-arm64-personal-test.apk")
    sha = subprocess.check_output(["git", "rev-parse", "HEAD"], text=True, encoding="utf-8").strip()
    info = {
        "upstream_repository": "https://github.com/AAswordman/Operit2",
        "source_commit": sha, "source_url": f"https://github.com/AAswordman/Operit2/tree/{sha}",
        "abi": "arm64-v8a", "build_channel": channel(),
        "version_code": os.environ.get("OPERIT2_BUILD_NUMBER"), "package_id": "app.operit",
        "android_dependency_compatibility": compatibility, "official_build": False, "device_tested": False,
        "signing": "Fixed personal update key" if os.environ.get("OPERIT2_BUILD_NUMBER") else "Personal test key, NOT the upstream author's signing key.",
        "update_warning": "Switching from an old one-time-signed APK needs backup/reinstallation. Future channel updates retain the configured fixed key.",
    }
    (DIST_DIR / "BUILD-INFO.json").write_text(json.dumps(info, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    wrapper = Path(__file__).resolve().parent.parent
    shutil.copy2(wrapper / (".github/workflows/update-android.yml" if os.environ.get("OPERIT2_BUILD_NUMBER") else ".github/workflows/operit2-android-test.yml"), DIST_DIR / "build-workflow.yml")
    if channel() == "enhanced":
        shutil.copy2(wrapper / ".github/workflows/enhanced-android.yml", DIST_DIR / "enhanced-workflow.yml")
    shutil.copy2(__file__, DIST_DIR / "build_arm64.py")
    shutil.copy2(wrapper / "builder/verify_apk.py", DIST_DIR / "verify_apk.py")
    shutil.copy2(root / "LICENSE", DIST_DIR / "UPSTREAM-LICENSE")

if __name__ == "__main__":
    build()
