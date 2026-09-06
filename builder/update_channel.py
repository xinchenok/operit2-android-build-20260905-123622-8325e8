"""Resolve upstream once and publish verified, consistently signed Android channels."""
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
from channel_policy import channel, modification_identity

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
    match = re.search(r"(?m)^version:\s*[^\r\n+]+\+(\d+)\s*$", pubspec)
    if not match:
        raise RuntimeError("Cannot read upstream Android versionCode from pubspec.yaml")
    value = max(int(match[1]), 1_000_000_000 + run_number, int(last.get("version_code", 0)) + 1)
    if not 0 < value <= 2_100_000_000:
        raise RuntimeError("Android versionCode is out of range")
    return value

def published_builds(repo: str) -> list[dict]:
    """Both channels contribute to a shared, monotonically increasing versionCode."""
    builds = []
    page = 1
    while True:
        releases = api(f"repos/{repo}/releases?per_page=100&page={page}")
        for release in releases:
            tag = release.get("tag_name", "")
            if release.get("draft") or not re.fullmatch(r"android-(?:(?:original|enhanced)-)?[0-9]+-[0-9a-f]+", tag):
                continue
            assets = [a for a in release.get("assets", []) if a["name"] == "UPDATE.json"]
            if len(assets) != 1:
                raise RuntimeError(f"Published Android release {tag} is missing its update metadata")
            inferred_channel = tag.split('-')[1] if tag.split('-')[1] in ("original", "enhanced") else "legacy-personal"
            code = int(re.search(r"(?:^|-)([0-9]+)-[0-9a-f]+$", tag)[1])
            builds.append({"tag": tag, "channel": inferred_channel, "version_code": code,
                           "metadata_url": assets[0]["browser_download_url"]})
        if len(releases) < 100:
            break
        page += 1
    return sorted(builds, key=lambda row: row["version_code"], reverse=True)

def release_state(repo: str, fingerprint: str, kind: str) -> tuple[dict, int]:
    records = published_builds(repo)
    high_water = max((row["version_code"] for row in records), default=0)
    selected = next((row for row in records if row["channel"] == kind), None)
    candidates = {row['tag']: row for row in [records[0] if records else None, selected] if row}
    last = {}
    for tag, row in candidates.items():
        meta = get(row["metadata_url"], api=False)
        if meta.get("certificate_sha256") != fingerprint:
            raise RuntimeError("Signing certificate differs from the existing update channel; retain the configured key")
        if int(meta.get("version_code", -1)) != row["version_code"]:
            raise RuntimeError(f"Release version metadata does not match {tag}")
        if row is selected:
            if meta.get("build_channel") != kind:
                raise RuntimeError(f"Release {tag} has inconsistent channel metadata")
            last = meta
    return last, high_water

def resolve() -> None:
    raw = os.environ.get("OPERIT2_UPDATE_SIGNING", "").strip()
    if not raw:
        message = ("Fixed signing is not configured. In GitHub Settings > Secrets and variables > Actions, "
                   "add OPERIT2_UPDATE_SIGNING once using the complete private signing bundle. "
                   "No desktop script is needed. No Android build was started.")
        summary = os.environ.get("GITHUB_STEP_SUMMARY")
        repo = os.environ.get("GITHUB_REPOSITORY", "")
        if summary:
            with open(summary, "a", encoding="utf-8") as stream:
                stream.write("## First-time signing setup\n" + message + "\n\n")
                stream.write(f"[Add the repository Secret](https://github.com/{repo}/settings/secrets/actions/new)\n")
        raise RuntimeError(message)
    try:
        signing = json.loads(raw)
        fingerprint = signing["certificate_sha256"].lower()
        if signing.get("schema") != 1 or not signing["pfx_base64"] or not signing["password"] or not re.fullmatch(r"[0-9a-f]{64}", fingerprint):
            raise ValueError()
    except (ValueError, KeyError, TypeError, AttributeError):
        raise RuntimeError("Fixed signing secret is malformed; restore the complete private signing bundle in OPERIT2_UPDATE_SIGNING. Do not generate a replacement key for an existing update channel") from None
    owner_repo = os.environ["GITHUB_REPOSITORY"]
    upstream = api(f"repos/{UPSTREAM}")
    branch = upstream["default_branch"]
    commit = api(f"repos/{UPSTREAM}/commits/{quote(branch, safe='')}")
    sha = commit["sha"]
    if not re.fullmatch(r"[0-9a-f]{40}", sha):
        raise RuntimeError("GitHub did not return a full upstream commit ID")
    kind = channel()
    last, high_water = release_state(owner_repo, fingerprint, kind)
    legacy_sha = ""
    if kind == "enhanced":
        legacy = api("repos/AAswordman/Operit")
        legacy_sha = api(f"repos/AAswordman/Operit/commits/{quote(legacy['default_branch'], safe='')}")["sha"]
        if not re.fullmatch(r"[0-9a-f]{40}", legacy_sha):
            raise RuntimeError("Invalid resolved Operit legacy commit")
    os.environ["LEGACY_SHA"] = legacy_sha
    tools = tool_versions(source_file("apps/flutter/app/.fvmrc", sha), source_file(".github/workflows/android-flutter-build.yml", sha))
    builder_sha = os.environ["GITHUB_SHA"]
    identity = modification_identity()
    build = not (last.get("source_commit") == sha and last.get("builder_commit") == builder_sha
                 and last.get("application_identity") == identity
                 and int(last.get("version_code", 0)) == high_water)
    code = version_code(source_file("apps/flutter/app/pubspec.yaml", sha),
                        {"version_code": high_water}, int(os.environ["GITHUB_RUN_NUMBER"]))
    tool_key = hashlib.sha256(json.dumps(tools, sort_keys=True).encode()).hexdigest()[:16]
    app_key = hashlib.sha256(json.dumps(identity, sort_keys=True).encode()).hexdigest()[:16]
    entries = api(f"repos/{UPSTREAM}/contents/tools/android-runtime?ref={sha}")
    runtime_inputs = sorted((entry["path"], entry["sha"]) for entry in entries)
    runtime_key = hashlib.sha256(json.dumps(runtime_inputs).encode()).hexdigest()[:16]
    outputs = {
        "source_sha": sha, "should_build": str(build).lower(),
        "build_number": str(code), "flutter_version": tools["FLUTTER_VERSION"],
        "toolchain": json.dumps(tools, separators=(",", ":")), "tool_key": tool_key,
        "runtime_key": runtime_key, "certificate_sha256": fingerprint,
        "app_key": app_key, "legacy_sha": legacy_sha, "channel": kind,
    }
    with open(os.environ["GITHUB_OUTPUT"], "a", encoding="utf-8") as stream:
        for key, value in outputs.items():
            stream.write(f"{key}={value}\n")
    print(f"Channel {kind}; Upstream {branch}: {sha}; {'building' if build else 'already built; download latest release'}")
    with open(os.environ["GITHUB_STEP_SUMMARY"], "a", encoding="utf-8") as stream:
        stream.write(f"## {kind.title()} update target\nUpstream: `{UPSTREAM}` / `{branch}` / `{sha}`\n\n")
        stream.write(f"{'Building a new APK entirely on GitHub' if build else 'No rebuild needed; the latest successful release already matches'}.\n\n")
        if not build:
            stream.write(f"[Download the channel APK](https://github.com/{owner_repo}/releases)\n")

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
    kind = channel()
    if info.get("build_channel") != kind:
        raise RuntimeError("APK channel is different from this workflow")
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
        "build_channel": kind, "application_identity": info["application_identity"],
    }
    published_name = f"operit2-{kind}-arm64.apk"
    apk.replace(dist / published_name)
    (dist / "SHA256SUMS").write_text(f"{digest}  {published_name}\n", encoding="ascii")
    metadata["apk_filename"] = published_name
    (dist / "UPDATE.json").write_text(json.dumps(metadata, indent=2) + "\n", encoding="utf-8")
    tag = f"android-{kind}-{code}-{sha[:7]}"
    repo = os.environ["GITHUB_REPOSITORY"]
    notes = (f"Unofficial {kind.upper()} ARM64 build of AAswordman/Operit2.\n\n"
             f"Upstream source: {sha}\nBuilder: {os.environ['GITHUB_SHA']}\n"
             f"Android versionCode: {code}\n\nFixed personal signing key. Not device-tested. "
             "An APK signed with a different key cannot be overwritten by this build; back up app data before switching signing channels.\n")
    notes_path = Path("release-notes.txt"); notes_path.write_text(notes, encoding="utf-8")
    common = ["--repo", repo]
    existing = api(f"repos/{repo}/releases/tags/{tag}", missing=True)
    if existing and not existing.get("draft"):
        print("This verified release has already been published")
        return
    if not existing:
        subprocess.run(["gh", "release", "create", tag, *common, "--draft", "--target", os.environ["GITHUB_SHA"],
                        "--title", f"Operit2 {kind.title()} ARM64 {sha[:7]} ({code})", "--notes-file", str(notes_path)], check=True)
    files = [str(p) for p in sorted(dist.iterdir()) if p.is_file()]
    subprocess.run(["gh", "release", "upload", tag, *common, *files, "--clobber"], check=True)
    subprocess.run(["gh", "release", "edit", tag, *common, "--draft=false", "--latest" if kind == "original" else "--latest=false"], check=True)
    print(f"Published {tag}")
    summary = os.environ.get("GITHUB_STEP_SUMMARY")
    if summary:
        with open(summary, "a", encoding="utf-8") as stream:
            stream.write(f"## APK ready\n[Download this build](https://github.com/{repo}/releases/tag/{tag})\n\n")
            stream.write(f"Source: `{sha}`\n\nAndroid versionCode: `{code}`\n\nCertificate SHA-256: `{certificate}`\n")

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("stage", choices=("resolve", "publish"))
    args = parser.parse_args()
    {"resolve": resolve, "publish": publish}[args.stage]()
