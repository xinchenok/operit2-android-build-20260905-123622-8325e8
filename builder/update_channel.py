"""Resolve upstream once and publish only verified, consistently signed Android builds."""
from __future__ import annotations
import argparse
import base64
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import time
from urllib.error import HTTPError, URLError
from urllib.parse import quote
from urllib.request import Request, urlopen

UPSTREAM = "AAswordman/Operit2"
APK = "operit2-android-arm64-personal-test.apk"
TOOL_DEFAULTS = {
    "RUST_TOOLCHAIN_VERSION": "1.95.0", "FVM_VERSION": "4.1.2",
    "TYPESCRIPT_VERSION": "5.9.3", "TERSER_VERSION": "5.44.0",
    "WASM_BINDGEN_VERSION": "0.2.122", "WASI_SDK_VERSION": "20.0",
    "QUICKJS_WASM_SYS_WASI_SDK_MAJOR_VERSION": "20",
    "QUICKJS_WASM_SYS_WASI_SDK_MINOR_VERSION": "0",
}

def get(url: str, *, api: bool = True, missing: bool = False):
    headers = {"User-Agent": "Operit2-personal-update", "Accept": "application/vnd.github+json" if api else "application/json"}
    token = os.environ.get("GH_TOKEN", "")
    if api and token:
        headers["Authorization"] = "Bearer " + token
    for attempt in range(4):
        try:
            with urlopen(Request(url, headers=headers), timeout=45) as response:
                return json.load(response)
        except HTTPError as error:
            if error.code == 404 and missing:
                return None
            if error.code not in (429, 500, 502, 503, 504) or attempt == 3:
                raise
        except (URLError, TimeoutError):
            if attempt == 3:
                raise
        time.sleep(2 ** attempt)
    raise RuntimeError("GitHub request did not complete")

def api(path: str, *, missing: bool = False):
    return get("https://api.github.com/" + path, missing=missing)

def source_file(path: str, sha: str) -> str:
    data = api(f"repos/{UPSTREAM}/contents/{path}?ref={sha}")
    return base64.b64decode(data["content"]).decode("utf-8-sig")

def tool_versions(fvm: str, workflow: str) -> dict[str, str]:
    flutter = str(json.loads(fvm)["flutter"])
    if not re.fullmatch(r"\d+\.\d+\.\d+(?:-[\w.]+)?", flutter):
        raise RuntimeError("Upstream FVM pin is not an exact supported Flutter version; no APK will be published")
    result = dict(TOOL_DEFAULTS, FLUTTER_VERSION=flutter)
    for key in TOOL_DEFAULTS:
        match = re.search(r"(?m)^\s+" + key + r"\s*:\s*['\"]?([0-9][A-Za-z0-9_.-]*)['\"]?\s*(?:#.*)?$", workflow)
        if match:
            result[key] = match.group(1)
    result["WASI_SDK_MAJOR"] = result["WASI_SDK_VERSION"].split(".")[0]
    return result

def version_code(pubspec: str, last: dict, run_number: int) -> int:
    # Parse the numeric Android build suffix, independently of versionName format.
    match = re.search(r"(?m)^version:\s*[^\r\n+]+\+(\d+)\s*$", pubspec)
    if not match:
        raise RuntimeError("Cannot read upstream Android versionCode from pubspec.yaml")
    value = max(int(match[1]), 1_000_000_000 + run_number, int(last.get("version_code", 0)) + 1)
    if not 0 < value <= 2_100_000_000:
        raise RuntimeError("Android versionCode is out of range")
    return value

def resolve() -> None:
    raw = os.environ.get("OPERIT2_UPDATE_SIGNING", "").strip()
    if not raw:
        raise RuntimeError("Fixed signing is not configured. Run Update-Operit2.cmd once on your Windows PC first. No Android build was started.")
    try:
        signing = json.loads(raw)
        fingerprint = signing["certificate_sha256"].lower()
        if signing.get("schema") != 1 or not signing["pfx_base64"] or not signing["password"] or not re.fullmatch(r"[0-9a-f]{64}", fingerprint):
            raise ValueError()
    except (ValueError, KeyError, TypeError, AttributeError):
        raise RuntimeError("Fixed signing secret is malformed; restore it from the updater's saved key") from None
    owner_repo = os.environ["GITHUB_REPOSITORY"]
    upstream = api(f"repos/{UPSTREAM}")
    branch = upstream["default_branch"]
    commit = api(f"repos/{UPSTREAM}/commits/{quote(branch, safe='')}")
    sha = commit["sha"]
    if not re.fullmatch(r"[0-9a-f]{40}", sha):
        raise RuntimeError("GitHub did not return a full upstream commit ID")
    last = {}
    release = api(f"repos/{owner_repo}/releases/latest", missing=True)
    if release:
        assets = [a for a in release["assets"] if a["name"] == "UPDATE.json"]
        if len(assets) != 1:
            raise RuntimeError("Existing latest release has no unambiguous UPDATE.json; it was not replaced")
        last = get(assets[0]["browser_download_url"], api=False)
        if last.get("certificate_sha256") != fingerprint:
            raise RuntimeError("Signing certificate differs from the last release. Restore the existing key; do not generate a new one")
    tools = tool_versions(source_file("apps/flutter/app/.fvmrc", sha), source_file(".github/workflows/android-flutter-build.yml", sha))
    builder_sha = os.environ["GITHUB_SHA"]
    build = not (last.get("source_commit") == sha and last.get("builder_commit") == builder_sha)
    code = version_code(source_file("apps/flutter/app/pubspec.yaml", sha), last, int(os.environ["GITHUB_RUN_NUMBER"]))
    tool_key = hashlib.sha256(json.dumps(tools, sort_keys=True).encode()).hexdigest()[:16]
    # Runtime inputs are independent of app UI changes. Cache only the exact input trees.
    entries = api(f"repos/{UPSTREAM}/contents/tools/android-runtime?ref={sha}")
    runtime_inputs = sorted((entry["path"], entry["sha"]) for entry in entries)
    runtime_key = hashlib.sha256(json.dumps(runtime_inputs).encode()).hexdigest()[:16]
    outputs = {
        "source_sha": sha, "should_build": str(build).lower(),
        "build_number": str(code), "flutter_version": tools["FLUTTER_VERSION"],
        "toolchain": json.dumps(tools, separators=(",", ":")), "tool_key": tool_key,
        "runtime_key": runtime_key, "certificate_sha256": fingerprint,
    }
    with open(os.environ["GITHUB_OUTPUT"], "a", encoding="utf-8") as stream:
        for key, value in outputs.items():
            stream.write(f"{key}={value}\n")
    print(f"Upstream {branch}: {sha}; {'building' if build else 'already built; download latest release'}")
    with open(os.environ["GITHUB_STEP_SUMMARY"], "a", encoding="utf-8") as stream:
        stream.write(f"## Update target\nUpstream: `{UPSTREAM}` / `{branch}` / `{sha}`\n\n")
        stream.write(f"{'Building a new APK' if build else 'No rebuild needed; the latest successful release already matches'}.\n")


def certificate_digest(signature: str) -> str:
    values = re.findall(r"(?mi)^(?:Signer #\d+|V\d(?:\.\d+)? Signer):?\s+certificate SHA-256 digest:\s*([0-9a-f]{64})\s*$", signature)
    unique = {value.lower() for value in values}
    if len(unique) != 1:
        raise RuntimeError("Expected exactly one APK signing certificate fingerprint")
    return unique.pop()


def publish() -> None:
    dist = Path("dist")
    apk = dist / APK
    info = json.loads((dist / "BUILD-INFO.json").read_text(encoding="utf-8"))
    sha = os.environ["SOURCE_SHA"]
    code = int(os.environ["BUILD_NUMBER"])
    if info["source_commit"] != sha or int(info["version_code"]) != code:
        raise RuntimeError("APK build provenance does not match this update request")
    digest = hashlib.sha256(apk.read_bytes()).hexdigest()
    sums = (dist / "SHA256SUMS").read_text(encoding="utf-8")
    if f"{digest}  {APK}" not in sums:
        raise RuntimeError("Downloaded APK does not match its checksum")
    signature = (dist / "signature.txt").read_text(encoding="utf-8-sig")
    certificate = certificate_digest(signature)
    if certificate != os.environ["CERTIFICATE_SHA256"].lower():
        raise RuntimeError("APK signing certificate does not match the fixed update key")
    metadata = {
        "schema": 1, "source_repository": UPSTREAM, "source_commit": sha,
        "builder_commit": os.environ["GITHUB_SHA"], "version_code": code,
        "certificate_sha256": certificate, "apk_sha256": digest,
        "apk_filename": APK, "run_id": os.environ["GITHUB_RUN_ID"],
        "official": False, "device_tested": False,
    }
    (dist / "UPDATE.json").write_text(json.dumps(metadata, indent=2) + "\n", encoding="utf-8")
    tag = f"android-{code}-{sha[:7]}"
    repo = os.environ["GITHUB_REPOSITORY"]
    notes = ("Unofficial personal ARM64 build of AAswordman/Operit2.\n\n"
             f"Upstream source: {sha}\nBuilder: {os.environ['GITHUB_SHA']}\n"
             f"Android versionCode: {code}\n\nFixed personal signing key. Not device-tested. "
             "The old one-time-signed APK needs a one-time backup/reinstall before switching to this channel.\n")
    notes_path = Path("release-notes.txt"); notes_path.write_text(notes, encoding="utf-8")
    common = ["--repo", repo]
    existing = api(f"repos/{repo}/releases/tags/{tag}", missing=True)
    if existing and not existing.get("draft"):
        print("This verified release has already been published")
        return
    if not existing:
        subprocess.run(["gh", "release", "create", tag, *common, "--draft", "--target", os.environ["GITHUB_SHA"],
                        "--title", f"Operit2 ARM64 {sha[:7]} ({code})", "--notes-file", str(notes_path)], check=True)
    files = [str(p) for p in sorted(dist.iterdir()) if p.is_file()]
    subprocess.run(["gh", "release", "upload", tag, *common, *files, "--clobber"], check=True)
    subprocess.run(["gh", "release", "edit", tag, *common, "--draft=false", "--latest"], check=True)
    print(f"Published {tag}")

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("stage", choices=("resolve", "publish"))
    args = parser.parse_args()
    {"resolve": resolve, "publish": publish}[args.stage]()
