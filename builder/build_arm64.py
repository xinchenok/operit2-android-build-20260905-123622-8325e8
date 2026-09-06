"""Build upstream Operit2 for ARM64 with its existing Android build helpers.

Run from the upstream repository root on the Windows GitHub Actions runner.
This script has not been integration-tested in the current chat environment.
"""
from __future__ import annotations

import json
import os
import shutil
import subprocess
import sys
from pathlib import Path


# Application sources stay unchanged; the dependency build-script backport is documented.
from prepare_android import patch_dynamic_color

def build() -> None:
    root = Path.cwd()
    sys.path.insert(0, str(root / "tools" / "build_scripts"))
    from common import (  # type: ignore[import-not-found]
        DIST_DIR,
        FLUTTER_APP_DIR,
        copy_required_file,
        flutter_command,
        flutter_pub_get,
        prepare_web_access_embedded_assets,
        run,
        staged_non_ohos_flutter_dependencies,
    )
    from build_flutter_android import (  # type: ignore[import-not-found]
        configure_android_flutter_sdk,
        ensure_android_signing,
    )

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
            command.extend(["--build-number", build_number])
        run(
            command,
            cwd=FLUTTER_APP_DIR,
        )

    apk = FLUTTER_APP_DIR / "build/app/outputs/flutter-apk/app-arm64-v8a-release.apk"
    copy_required_file(apk, DIST_DIR / "operit2-android-arm64-personal-test.apk")
    sha = subprocess.check_output(
        ["git", "rev-parse", "HEAD"], text=True, encoding="utf-8"
    ).strip()
    info = {
        "upstream_repository": "https://github.com/AAswordman/Operit2",
        "source_commit": sha,
        "source_url": f"https://github.com/AAswordman/Operit2/tree/{sha}",
        "abi": "arm64-v8a",
        "version_code": os.environ.get("OPERIT2_BUILD_NUMBER"),
        "package_id": "app.operit",
        "android_dependency_compatibility": compatibility,
        "official_build": False,
        "device_tested": False,
        "signing": "Fixed personal update key" if os.environ.get("OPERIT2_BUILD_NUMBER") else "Personal test key, NOT the upstream author's signing key.",
        "update_warning": "Switching from an old one-time-signed APK needs backup/reinstallation. "
                          "Future channel updates retain the configured fixed key.",
    }
    (DIST_DIR / "BUILD-INFO.json").write_text(
        json.dumps(info, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
    )
    wrapper = Path(__file__).resolve().parent.parent
    shutil.copy2(wrapper / (".github/workflows/update-android.yml" if os.environ.get("OPERIT2_BUILD_NUMBER") else ".github/workflows/operit2-android-test.yml"), DIST_DIR / "build-workflow.yml")
    shutil.copy2(__file__, DIST_DIR / "build_arm64.py")
    shutil.copy2(wrapper / "builder/verify_apk.py", DIST_DIR / "verify_apk.py")
    shutil.copy2(root / "LICENSE", DIST_DIR / "UPSTREAM-LICENSE")


if __name__ == "__main__":
    build()
