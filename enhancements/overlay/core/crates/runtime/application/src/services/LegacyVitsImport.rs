//! Imports an existing, Sherpa-compatible Operit 1 VITS package without changing
//! the active TTS configuration. Run this on the legacy host job worker.
use operit_host_api::{FileSystemHost, RuntimeStorageHost, RuntimeStorageWriteHost};
use std::collections::{BTreeMap, BTreeSet};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use operit_host_api::HostManager::HostManager;
use operit_host_api::TimeUtils::currentTimeMillis;
use operit_local_models::LocalModelCatalog::LocalModelCatalog;
use operit_local_models::LocalModelManifest::{LocalModelDriver, LocalModelFile};
use operit_local_models::LocalModelRegistry::InstalledLocalModel;
use operit_local_models::LocalModelStorage::buildLocalModelStoragePath;
use operit_model::TtsConfig::TtsConfig;
use operit_tools::files::PathMapper::PathMapper;
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::data::preferences::TtsConfigManager::TtsConfigManager;
use crate::services::LocalModelService::LocalModelService;

const MAX_PACKAGE_BYTES: u64 = 4 * 1024 * 1024 * 1024;
const MAX_TEXT_BYTES: u64 = 32 * 1024 * 1024;

struct Scratch {
    path: PathBuf,
    host: Arc<dyn FileSystemHost>,
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = self.host.deleteFile(&self.path.to_string_lossy(), true);
    }
}
struct HostFiles {
    fs: Arc<dyn FileSystemHost>,
    storage: Arc<dyn RuntimeStorageHost>,
    writers: Arc<dyn RuntimeStorageWriteHost>,
    root: PathBuf,
    scratch: PathBuf,
}
impl HostFiles {
    fn info(&self, path: &Path) -> Result<operit_host_api::FileInfo, String> {
        self.fs.fileInfo(&path.to_string_lossy()).map_err(err)
    }
    fn size(&self, path: &Path) -> Result<u64, String> {
        let info = self.info(path)?;
        if !info.exists || info.fileType != "file" || info.size < 0 {
            return Err(format!("VITS 文件不可读：{}", path.display()));
        }
        Ok(info.size as u64)
    }
    fn mkdir(&self, path: &Path) -> Result<(), String> {
        self.fs
            .makeDirectory(&path.to_string_lossy(), true)
            .map_err(err)
    }
    fn storage_path(&self, path: &Path) -> Result<String, String> {
        let relative = path
            .strip_prefix(&self.root)
            .map_err(|_| "VITS 暂存文件超出运行时目录".to_string())?;
        if relative
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
        {
            return Err("VITS 运行时相对路径无效".into());
        }
        Ok(format!(
            "runtime/{}",
            relative.to_string_lossy().replace('\\', "/")
        ))
    }
    fn reader(&self, source: &Path) -> Result<std::io::BufReader<HostRangeReader>, String> {
        let size = self.size(source)?;
        if size > MAX_PACKAGE_BYTES {
            return Err("VITS 模型或压缩文件超过 4 GiB".into());
        }
        // FileSystemHost owns copying external files; range reads use the existing
        // RuntimeStorageHost contract. Neither operation loads the entire file.
        let (path, cleanup) = if source.starts_with(&self.root) {
            (source.to_path_buf(), None)
        } else {
            let temporary = self
                .scratch
                .join(format!("reader-{}", uuid::Uuid::new_v4()));
            self.fs
                .copyFile(
                    &source.to_string_lossy(),
                    &temporary.to_string_lossy(),
                    false,
                )
                .map_err(err)?;
            (
                temporary.clone(),
                Some(Scratch {
                    path: temporary,
                    host: self.fs.clone(),
                }),
            )
        };
        let length = self.size(&path)?;
        if length != size {
            return Err("VITS 源文件在复制期间发生改变，请停止修改后重试".into());
        }
        Ok(std::io::BufReader::with_capacity(
            64 * 1024,
            HostRangeReader {
                storage: self.storage.clone(),
                path: self.storage_path(&path)?,
                length,
                position: 0,
                _cleanup: cleanup,
            },
        ))
    }
}
struct HostRangeReader {
    storage: Arc<dyn RuntimeStorageHost>,
    path: String,
    length: u64,
    position: u64,
    _cleanup: Option<Scratch>,
}
impl Read for HostRangeReader {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        let count = (self.length - self.position).min(buffer.len().min(64 * 1024) as u64) as usize;
        if count == 0 {
            return Ok(0);
        }
        let bytes = self
            .storage
            .readBytesRange(&self.path, self.position, count)
            .map_err(|e| std::io::Error::other(e.to_string()))?;
        if bytes.len() != count {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "VITS runtime range read was truncated",
            ));
        }
        buffer[..count].copy_from_slice(&bytes);
        self.position += count as u64;
        Ok(count)
    }
}
impl Seek for HostRangeReader {
    fn seek(&mut self, offset: SeekFrom) -> std::io::Result<u64> {
        let next = match offset {
            SeekFrom::Start(position) => position as i128,
            SeekFrom::Current(delta) => self.position as i128 + delta as i128,
            SeekFrom::End(delta) => self.length as i128 + delta as i128,
        };
        if next < 0 || next > self.length as i128 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "VITS seek is outside the file",
            ));
        }
        self.position = next as u64;
        Ok(self.position)
    }
}

fn err(error: impl std::fmt::Display) -> String {
    error.to_string()
}

/// Returns a ready-to-bind configuration only after all files and the existing
/// native engine are installed. Failure never writes the current TTS preference.
pub fn prepare(host: &HostManager, request: &Value) -> Result<TtsConfig, String> {
    let mut config = TtsConfigManager::getInstance().getCurrentTtsConfig()?;
    let fs_host = host
        .fileSystemHost
        .clone()
        .ok_or("VITS 导入需要原生文件系统 Host")?;
    let package = request["tts_vits_package_path"]
        .as_str()
        .unwrap_or("")
        .trim();
    if package.is_empty() {
        return Err("VITS 模型包路径为空；请设置应用可读取的本地模型目录或 ZIP 文件".into());
    }
    let source = if package.starts_with("file:") {
        // `Url::to_file_path` only exists on native file-system targets.
        // Web Access forwards imports to a native Core; its wasm build must
        // still compile without pretending the browser can read local paths.
        #[cfg(any(
            unix,
            windows,
            target_os = "redox",
            target_os = "wasi",
            target_os = "hermit"
        ))]
        {
            url::Url::parse(package)
                .map_err(err)?
                .to_file_path()
                .map_err(|_| "VITS file URL 无法转换成本地路径".to_string())?
        }
        #[cfg(not(any(
            unix,
            windows,
            target_os = "redox",
            target_os = "wasi",
            target_os = "hermit"
        )))]
        {
            return Err("本地 VITS 模型导入需要连接原生 Core 服务".into());
        }
    } else {
        PathBuf::from(package)
    };
    let source = if source.to_string_lossy().starts_with("/app/")
        || source.to_string_lossy().starts_with("/mnt/")
    {
        let storage = host
            .runtimeStorageHost
            .as_ref()
            .ok_or("VITS 路径映射需要运行时存储")?;
        PathBuf::from(
            PathMapper::new(
                storage.runtimeRootDir().ok_or("VITS 运行时目录未配置")?,
                storage.workspaceRootDir().ok_or("VITS 工作区目录未配置")?,
            )
            .resolve(&source.to_string_lossy())?
            .physicalPath,
        )
    } else {
        source
    };
    let source = PathBuf::from(
        fs_host
            .canonicalizePath(&source.to_string_lossy())
            .map_err(|e| format!("无法读取 VITS 模型包 {}：{e}", source.display()))?,
    );
    let options: Value = match request.get("tts_vits_options") {
        None | Some(Value::Null) => serde_json::json!({}),
        Some(Value::String(value)) if value.trim().is_empty() => serde_json::json!({}),
        Some(Value::String(value)) => serde_json::from_str(value).map_err(err)?,
        Some(value) => value.clone(),
    };
    let options = options
        .as_object()
        .ok_or("tts_vits_options 必须是 JSON 对象")?;
    // These old options alter tensor layouts or tokenization. Silently ignoring
    // them can produce invalid tensors or a native fatal error, not just a new voice.
    for (key, value) in options {
        if !value.is_string() && !matches!(key.as_str(), "sample_rate" | "speaker_count") {
            return Err(format!("旧 VITS 选项 {key} 必须是字符串"));
        }
        if !matches!(
            key.as_str(),
            "model_path"
                | "config_path"
                | "lexicon_path"
                | "tokens_path"
                | "sample_rate"
                | "speaker_count"
                | "locale"
                | "frontend"
        ) {
            return Err(format!("旧 VITS 选项 {key} 需要自定义 ONNX 推理；当前 Sherpa 驱动无法等价执行，原语音配置保持不变"));
        }
    }
    if options
        .get("frontend")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty() && *s != "lexicon")
        .is_some()
    {
        return Err("此旧 VITS 前端需要自定义分词；当前仅导入带词典的 Sherpa 兼容 VITS 包".into());
    }
    let root = host
        .runtimeStorageHost
        .as_ref()
        .and_then(|s| s.runtimeRootDir())
        .ok_or("VITS 导入需要已初始化的运行时存储")?;
    let root = PathBuf::from(
        fs_host
            .canonicalizePath(&root.to_string_lossy())
            .map_err(err)?,
    );
    let scratch = Scratch {
        path: root
            .join("cache")
            .join(format!("legacy-vits-{}", uuid::Uuid::new_v4())),
        host: fs_host.clone(),
    };
    fs_host
        .makeDirectory(&scratch.path.to_string_lossy(), true)
        .map_err(err)?;
    let access = HostFiles {
        fs: fs_host,
        storage: host
            .runtimeStorageHost
            .clone()
            .ok_or("VITS 导入需要运行时存储 Host")?,
        writers: host
            .runtimeStorageWriteHost
            .clone()
            .ok_or("VITS 导入需要流式写入 Host")?,
        root: root.clone(),
        scratch: scratch.path.clone(),
    };
    let package_root = if access.info(&source)?.fileType == "directory" {
        source.clone()
    } else {
        let extracted = scratch.path.join("unpacked");
        unpack(&access, &source, &extracted)?;
        extracted
    };
    let mut files = Vec::new();
    list_files(&access, &package_root, &mut files, 0)?;
    let package_manifest = unique(
        &files,
        |p| p.file_name().is_some_and(|s| s == "operit-vits-tts.json"),
        false,
    )?;
    let declaration = package_manifest
        .as_ref()
        .map(|p| read_json(&access, p))
        .transpose()?
        .unwrap_or(Value::Null);
    reject_custom_frontend(&declaration)?;
    let model = choose_file(
        &access,
        &package_root,
        &declaration,
        options,
        "model",
        "model_path",
        &files,
        |p| {
            p.extension()
                .and_then(|s| s.to_str())
                .is_some_and(|s| s.eq_ignore_ascii_case("onnx"))
        },
    )?
    .ok_or("VITS 包未找到 ONNX 模型；多个模型时请填写 model_path")?;
    let metadata = onnx_metadata(&access, &model)?;
    for key in [
        "model_type",
        "sample_rate",
        "n_speakers",
        "language",
        "comment",
    ] {
        if !metadata.contains_key(key) {
            return Err(format!("旧 ONNX 缺少 Sherpa 必需元数据 {key}；不能直接加载此自定义 VITS 模型，原配置保持不变"));
        }
    }
    if metadata["model_type"] != "vits" {
        return Err("此 ONNX 模型不是 Sherpa VITS 模型".into());
    }
    if !metadata.get("frontend").is_none_or(|v| v.is_empty())
        || metadata.get("jieba").is_some_and(|v| v != "0")
        || metadata.get("has_g2pw").is_some_and(|v| v != "0")
        || metadata["comment"].contains("melo")
    {
        return Err("此 VITS 包还需要专用分词/字典驱动，不能按通用词典 VITS 导入".into());
    }
    for key in [
        "add_blank",
        "speaker_id",
        "version",
        "num_emotions",
        "blank_id",
        "bos_id",
        "eos_id",
        "use_eos_bos",
        "pad_id",
    ] {
        if let Some(value) = metadata.get(key) {
            if value.parse::<u32>().is_err() {
                return Err(format!("ONNX 元数据 {key} 必须是非负整数"));
            }
        }
    }
    let speakers = metadata["n_speakers"].parse::<i32>().map_err(err)?;
    let sample_rate = metadata["sample_rate"].parse::<u32>().map_err(err)?;
    if !(1..=4096).contains(&speakers) || !(8000..=384000).contains(&sample_rate) {
        return Err("VITS 声线数量或采样率无效".into());
    }
    for (key, expected) in [
        ("speaker_count", speakers.to_string()),
        ("sample_rate", sample_rate.to_string()),
    ] {
        if let Some(value) = options.get(key) {
            if value
                .as_str()
                .map(str::to_string)
                .unwrap_or_else(|| value.to_string())
                != expected
            {
                return Err(format!("旧 VITS 选项 {key} 与 ONNX 模型元数据不一致"));
            }
        }
    }
    let speaker = match request.get("tts_vits_speaker_id") {
        None | Some(Value::Null) => "",
        Some(Value::String(value)) => value.trim(),
        Some(_) => return Err("tts_vits_speaker_id 必须是数字字符串".into()),
    };
    let speaker: i32 = if speaker.is_empty() {
        0
    } else {
        speaker.parse().map_err(err)?
    };
    if !(0..speakers).contains(&speaker) {
        return Err(format!("VITS speaker_id 必须在 0 到 {} 之间", speakers - 1));
    }
    let lexicon = choose_file(
        &access,
        &package_root,
        &declaration,
        options,
        "lexicon",
        "lexicon_path",
        &files,
        |p| {
            p.file_name()
                .and_then(|s| s.to_str())
                .is_some_and(|s| s.eq_ignore_ascii_case("lexicon.txt"))
        },
    )?
    .ok_or("VITS 包缺少 lexicon.txt；依赖 espeak 数据目录的 Piper 包不能用当前词典驱动代替")?;
    let token_file = choose_file(
        &access,
        &package_root,
        &declaration,
        options,
        "tokens",
        "tokens_path",
        &files,
        |p| {
            p.file_name()
                .and_then(|s| s.to_str())
                .is_some_and(|s| s.eq_ignore_ascii_case("tokens.txt"))
        },
    )?;
    let legacy_config = resolve_file(
        &access,
        &package_root,
        &declaration,
        options,
        "config",
        "config_path",
    )?;
    let legacy_config = match legacy_config {
        Some(path) => Some(path),
        None => {
            let candidate = PathBuf::from(format!("{}.json", model.to_string_lossy()));
            let info = access.info(&candidate)?;
            (info.exists && info.fileType == "file").then_some(candidate)
        }
    };
    let token_text = if let Some(path) = token_file {
        read_text(&access, &path)?
    } else {
        let path = legacy_config
            .as_ref()
            .ok_or("VITS 包缺少 tokens.txt 或带 token map 的 ONNX 配置")?;
        tokens_from_config(&read_json(&access, path)?)?
    };
    let lexicon_text = read_text(&access, &lexicon)?;
    validate_lexicon(&token_text, &lexicon_text)?;
    if let Some(path) = legacy_config {
        let value = read_json(&access, &path)?;
        reject_custom_frontend(&value)?;
        // The current stock driver has fixed inference defaults. Reject overrides
        // instead of claiming they were applied or changing the user's voice silently.
        for (key, default) in [
            ("noise_scale", 0.667),
            ("length_scale", 1.0),
            ("noise_w", 0.8),
        ] {
            if let Some(raw) = value["inference"].get(key) {
                let value = raw
                    .as_f64()
                    .or_else(|| raw.as_str().and_then(|s| s.parse::<f64>().ok()))
                    .ok_or_else(|| format!("VITS 配置 {key} 必须是数值"))?;
                if !value.is_finite() || (value - default).abs() > 0.00001 {
                    return Err(format!("此 VITS 包需要非默认 {key}，当前驱动无法等价迁移"));
                }
            }
        }
    }
    let staged = scratch.path.join("model");
    access.mkdir(&staged)?;
    access
        .fs
        .copyFile(
            &model.to_string_lossy(),
            &staged.join("model.onnx").to_string_lossy(),
            false,
        )
        .map_err(err)?;
    if onnx_metadata(&access, &staged.join("model.onnx"))? != metadata {
        return Err("VITS 模型在导入期间发生改变，请停止修改模型文件后重试".into());
    }
    access
        .fs
        .writeFileBytes(
            &staged.join("tokens.txt").to_string_lossy(),
            token_text.as_bytes(),
        )
        .map_err(err)?;
    access
        .fs
        .writeFileBytes(
            &staged.join("lexicon.txt").to_string_lossy(),
            lexicon_text.as_bytes(),
        )
        .map_err(err)?;
    let mut manifest = LocalModelCatalog::sherpaOnnxVitsTts();
    manifest.files = ["model.onnx", "tokens.txt", "lexicon.txt"]
        .into_iter()
        .map(|name| {
            let (sha256, byteSize) = digest_file(&access, &staged.join(name))?;
            Ok(LocalModelFile {
                relativePath: name.into(),
                sha256,
                byteSize,
                sourceId: "legacy-local".into(),
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let fingerprint = format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&manifest.files).map_err(err)?)
    );
    manifest.id = format!("legacy-vits-{}", &fingerprint[..16]);
    manifest.version = "1".into();
    manifest.displayName = format!(
        "Operit 1 VITS {}",
        source.file_stem().unwrap_or_default().to_string_lossy()
    );
    manifest.description = "从本地旧版 VITS 包导入的 Sherpa 兼容模型".into();
    manifest.license = "user-provided".into();
    manifest.homepage.clear();
    manifest.languages = vec![options
        .get("locale")
        .and_then(Value::as_str)
        .filter(|s| !s.trim().is_empty())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| metadata["language"].clone())];
    manifest.tags = vec!["legacy-import".into(), "vits".into()];
    manifest.sources.clear();
    manifest.driver = Some(LocalModelDriver::SherpaOnnxVits {
        model: "model.onnx".into(),
        lexicon: "lexicon.txt".into(),
        tokens: "tokens.txt".into(),
        ruleFsts: vec![],
        ruleFars: vec![],
        speakerCount: speakers,
    });
    let storage_path = buildLocalModelStoragePath(
        &manifest.kind,
        &manifest.engine,
        &manifest.id,
        &manifest.version,
    )
    .map_err(err)?;
    let target = root.join(
        storage_path
            .strip_prefix("runtime/")
            .ok_or("VITS 模型存储路径无效")?,
    );
    access.mkdir(target.parent().ok_or("VITS 模型存储目录无效")?)?;
    if access.info(&target)?.exists {
        for file in &manifest.files {
            let actual = digest_file(&access, &target.join(&file.relativePath))?;
            if actual != (file.sha256.clone(), file.byteSize) {
                return Err("已导入的 VITS 文件已改变；请在本地模型设置中删除后重新导入".into());
            }
        }
    } else {
        access
            .fs
            .moveFile(&staged.to_string_lossy(), &target.to_string_lossy())
            .map_err(err)?;
    }
    let model_key = manifest.registryKey();
    let now = currentTimeMillis();
    LocalModelService::getInstance(host)?.registerLegacyVitsModel(InstalledLocalModel {
        manifest,
        storagePath: storage_path,
        installedAtMs: now,
        verifiedAtMs: Some(now),
    })?;
    config.providerType = "LOCAL_MODEL".into();
    config.model = model_key;
    config.voice = speaker.to_string();
    config.responseFormat = "wav".into();
    config.endpoint.clear();
    config.apiKey.clear();
    config.headers.clear();
    config.requestBody.clear();
    config.responsePipeline.clear();
    Ok(config)
}

fn reject_custom_frontend(config: &Value) -> Result<(), String> {
    for section in [config, &config["options"], &config["inference"]] {
        let Some(fields) = section.as_object() else {
            continue;
        };
        for (key, value) in fields {
            if key == "frontend"
                && value
                    .as_str()
                    .is_none_or(|s| !s.is_empty() && s != "lexicon")
            {
                return Err("旧 VITS 包声明了当前驱动无法等价执行的 frontend".into());
            }
            if matches!(
                key.as_str(),
                "bos_token_ids"
                    | "bos_token_id"
                    | "eos_token_ids"
                    | "eos_token_id"
                    | "blank_token_id"
                    | "add_blank"
                    | "interleave_blank"
                    | "text_mode"
                    | "token_ids"
                    | "phoneme_ids"
                    | "ids_input"
                    | "input_ids_name"
                    | "length_input"
                    | "input_lengths_name"
                    | "scales_input"
                    | "scales_name"
                    | "sid_input"
                    | "speaker_input_name"
            ) {
                return Err(format!(
                    "旧 VITS 包声明的 {key} 需要自定义 ONNX 分词/张量映射，当前驱动不能等价执行"
                ));
            }
        }
    }
    Ok(())
}

fn read_text(access: &HostFiles, path: &Path) -> Result<String, String> {
    let size = access.size(path)?;
    if size > MAX_TEXT_BYTES {
        return Err(format!("VITS 文本配置文件过大：{}", path.display()));
    }
    let mut bytes = Vec::with_capacity(size as usize);
    access
        .reader(path)?
        .take(MAX_TEXT_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(err)?;
    if bytes.len() as u64 != size {
        return Err("VITS 文本文件在读取期间发生改变".into());
    }
    String::from_utf8(bytes).map_err(err)
}
fn read_json(access: &HostFiles, path: &Path) -> Result<Value, String> {
    serde_json::from_str(&read_text(access, path)?).map_err(err)
}
fn unique(
    files: &[PathBuf],
    matches: impl Fn(&Path) -> bool,
    required: bool,
) -> Result<Option<PathBuf>, String> {
    let mut found = files.iter().filter(|p| matches(p));
    let first = found.next().cloned();
    if found.next().is_some() {
        return Err("VITS 包存在多个同类文件，请用 tts_vits_options 明确相对路径".into());
    }
    if required && first.is_none() {
        return Err("VITS 包缺少必要文件".into());
    }
    Ok(first)
}
fn choose_file(
    access: &HostFiles,
    root: &Path,
    declaration: &Value,
    options: &serde_json::Map<String, Value>,
    field: &str,
    option: &str,
    files: &[PathBuf],
    matches: impl Fn(&Path) -> bool,
) -> Result<Option<PathBuf>, String> {
    match resolve_file(access, root, declaration, options, field, option)? {
        Some(file) => Ok(Some(file)),
        None => unique(files, matches, false),
    }
}
fn resolve_file(
    access: &HostFiles,
    root: &Path,
    declaration: &Value,
    options: &serde_json::Map<String, Value>,
    field: &str,
    option: &str,
) -> Result<Option<PathBuf>, String> {
    let raw = declaration
        .get(field)
        .and_then(Value::as_str)
        .or_else(|| options.get(option).and_then(Value::as_str))
        .filter(|s| !s.is_empty());
    let Some(raw) = raw else {
        return Ok(None);
    };
    let path = PathBuf::from(
        access
            .fs
            .canonicalizePath(&root.join(raw).to_string_lossy())
            .map_err(err)?,
    );
    let canonical_root = PathBuf::from(
        access
            .fs
            .canonicalizePath(&root.to_string_lossy())
            .map_err(err)?,
    );
    if !path.starts_with(canonical_root) || access.info(&path)?.fileType != "file" {
        return Err(format!("VITS 包文件必须位于包目录内：{raw}"));
    }
    Ok(Some(path))
}
fn list_files(
    access: &HostFiles,
    path: &Path,
    out: &mut Vec<PathBuf>,
    depth: usize,
) -> Result<(), String> {
    if depth > 16 {
        return Err("VITS 包目录层数过多".into());
    }
    let canonical_parent = PathBuf::from(
        access
            .fs
            .canonicalizePath(&path.to_string_lossy())
            .map_err(err)?,
    );
    for entry in access.fs.listFiles(&path.to_string_lossy()).map_err(err)? {
        let name = Path::new(&entry.name);
        if name.components().count() != 1
            || !matches!(name.components().next(), Some(Component::Normal(_)))
        {
            return Err("VITS 文件系统 Host 返回了无效文件名".into());
        }
        let child = canonical_parent.join(name);
        let canonical_child = PathBuf::from(
            access
                .fs
                .canonicalizePath(&child.to_string_lossy())
                .map_err(err)?,
        );
        if canonical_child != child {
            return Err("VITS 包不支持符号链接，请提供完整模型文件".into());
        }
        let info = access.info(&child)?;
        if info.fileType == "directory" {
            list_files(access, &child, out, depth + 1)?;
        } else if info.fileType == "file" {
            out.push(child);
        }
        if out.len() > 10000 {
            return Err("VITS 包文件过多".into());
        }
    }
    Ok(())
}
fn unpack(access: &HostFiles, source: &Path, target: &Path) -> Result<(), String> {
    let mut zip = zip::ZipArchive::new(access.reader(source)?)
        .map_err(|e| format!("VITS 包不是可读取的目录或 ZIP：{e}"))?;
    if zip.len() > 10000 {
        return Err("VITS ZIP 文件过多".into());
    }
    access.mkdir(target)?;
    let mut total = 0u64;
    let mut names = BTreeSet::new();
    for index in 0..zip.len() {
        let mut entry = zip.by_index(index).map_err(err)?;
        let relative = entry
            .enclosed_name()
            .ok_or("VITS ZIP 文件路径越界")?
            .to_path_buf();
        if !names.insert(relative.clone()) {
            return Err("VITS ZIP 存在重复路径".into());
        }
        if entry
            .unix_mode()
            .is_some_and(|mode| mode & 0o170000 == 0o120000)
        {
            return Err("VITS ZIP 不支持符号链接".into());
        }
        let path = target.join(relative);
        if entry.is_dir() {
            access.mkdir(&path)?;
            continue;
        }
        access.mkdir(path.parent().ok_or("VITS ZIP 路径无效")?)?;
        let mut writer = access
            .writers
            .createWriteSession(&access.storage_path(&path)?)
            .map_err(err)?;
        let outcome = (|| -> Result<(), String> {
            let mut buffer = [0u8; 64 * 1024];
            loop {
                let count = entry.read(&mut buffer).map_err(err)?;
                if count == 0 {
                    break;
                }
                total += count as u64;
                if total > MAX_PACKAGE_BYTES {
                    return Err("VITS ZIP 解压内容超过 4 GiB".into());
                }
                writer.writeChunk(&buffer[..count]).map_err(err)?;
            }
            Ok(())
        })();
        if let Err(error) = outcome {
            let _ = writer.discard();
            return Err(error);
        }
        writer.commitFast().map_err(err)?;
    }
    Ok(())
}
fn tokens_from_config(config: &Value) -> Result<String, String> {
    let map = ["phoneme_id_map", "token_id_map", "tokens", "vocab"]
        .iter()
        .find_map(|key| config[*key].as_object())
        .or_else(|| config["model"]["vocab"].as_object());
    let mut tokens = BTreeMap::new();
    if let Some(map) = map {
        for (symbol, ids) in map {
            let id = match ids.as_array() {
                Some(ids) if ids.len() == 1 => &ids[0],
                Some(_) => {
                    return Err(
                        "此旧 VITS token 对应多个 ID，不能等价转换为 Sherpa tokens.txt".into(),
                    )
                }
                None => ids,
            };
            let id = id
                .as_i64()
                .or_else(|| id.as_str().and_then(|s| s.parse().ok()))
                .ok_or("VITS token ID 无效")?;
            tokens.insert(symbol.clone(), id);
        }
    } else if let Some(symbols) = config["symbols"]
        .as_array()
        .or_else(|| config["model"]["symbols"].as_array())
    {
        for (id, symbol) in symbols.iter().enumerate() {
            tokens.insert(
                symbol.as_str().ok_or("VITS symbol 必须是字符串")?.into(),
                id as i64,
            );
        }
    }
    if tokens.is_empty() {
        return Err("VITS 配置没有可转换的 token map".into());
    }
    let mut text = String::new();
    for (symbol, id) in tokens {
        if id < 0 || symbol.contains(['\r', '\n', '\t']) || (symbol.contains(' ') && symbol != " ")
        {
            return Err("VITS token 无法用 Sherpa tokens.txt 表示".into());
        }
        if !symbol.is_empty() {
            text.push_str(&format!("{symbol} {id}\n"));
        }
    }
    Ok(text)
}
fn validate_lexicon(tokens: &str, lexicon: &str) -> Result<(), String> {
    let mut names = BTreeSet::new();
    for line in tokens.lines().filter(|line| !line.is_empty()) {
        let (name, id) = line.rsplit_once(' ').ok_or("tokens.txt 行格式无效")?;
        if id.trim().parse::<u32>().is_err() || !names.insert(name) {
            return Err("tokens.txt 的 token 或 ID 无效/重复".into());
        }
    }
    if names.is_empty() || lexicon.trim().is_empty() {
        return Err("VITS tokens 或词典为空".into());
    }
    for line in lexicon.lines().filter(|line| !line.trim().is_empty()) {
        let mut parts = line.split_whitespace();
        parts.next();
        let phonemes = parts.collect::<Vec<_>>();
        if phonemes.is_empty() || phonemes.iter().any(|p| !names.contains(p)) {
            return Err("VITS 词典包含未声明的 token 或缺少发音".into());
        }
    }
    Ok(())
}
fn digest_file(access: &HostFiles, path: &Path) -> Result<(String, u64), String> {
    let mut file = access.reader(path)?;
    let mut digest = Sha256::new();
    let mut total = 0;
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let count = file.read(&mut buffer).map_err(err)?;
        if count == 0 {
            break;
        }
        total += count as u64;
        if total > MAX_PACKAGE_BYTES {
            return Err("VITS 模型文件超过 4 GiB".into());
        }
        digest.update(&buffer[..count]);
    }
    Ok((format!("{:x}", digest.finalize()), total))
}

// ONNX ModelProto field 14 contains StringStringEntryProto metadata. Skip tensor
// payloads with seeks, so inspecting a large model does not allocate its weights.
fn onnx_metadata(access: &HostFiles, path: &Path) -> Result<BTreeMap<String, String>, String> {
    let mut file = access.reader(path)?;
    let end = access.size(path)?;
    if end > MAX_PACKAGE_BYTES {
        return Err("VITS 模型文件超过 4 GiB".into());
    }
    let mut result = BTreeMap::new();
    while file.stream_position().map_err(err)? < end {
        let key = varint(&mut file)?;
        let wire = key & 7;
        let size = match wire {
            0 => {
                varint(&mut file)?;
                continue;
            }
            1 => 8,
            2 => varint(&mut file)?,
            5 => 4,
            _ => return Err("ONNX protobuf 格式无效".into()),
        };
        let next = file
            .stream_position()
            .map_err(err)?
            .checked_add(size)
            .ok_or("ONNX 长度溢出")?;
        if next > end {
            return Err("ONNX 文件已截断".into());
        }
        if key >> 3 == 14 && wire == 2 {
            if size > 65536 || result.len() > 1024 {
                return Err("ONNX 元数据过大".into());
            }
            let mut bytes = vec![0u8; size as usize];
            file.read_exact(&mut bytes).map_err(err)?;
            let mut reader = std::io::Cursor::new(bytes);
            let mut pair = [String::new(), String::new()];
            while reader.position() < size {
                let field = varint(&mut reader)?;
                if field != 10 && field != 18 {
                    return Err("ONNX 元数据结构无效".into());
                }
                let len = varint(&mut reader)?;
                if len > size - reader.position() {
                    return Err("ONNX 元数据已截断".into());
                }
                let mut value = vec![0u8; len as usize];
                reader.read_exact(&mut value).map_err(err)?;
                pair[((field >> 3) - 1) as usize] = String::from_utf8(value).map_err(err)?;
            }
            if result.insert(pair[0].clone(), pair[1].clone()).is_some() {
                return Err("ONNX 存在重复元数据字段".into());
            }
        } else {
            file.seek(SeekFrom::Start(next)).map_err(err)?;
        }
    }
    Ok(result)
}
fn varint(reader: &mut impl Read) -> Result<u64, String> {
    let mut value = 0u64;
    for shift in (0..70).step_by(7) {
        let mut byte = [0u8; 1];
        reader.read_exact(&mut byte).map_err(err)?;
        if shift == 63 && byte[0] > 1 {
            return Err("ONNX varint 溢出".into());
        }
        value |= ((byte[0] & 127) as u64) << shift;
        if byte[0] & 128 == 0 {
            return Ok(value);
        }
    }
    Err("ONNX varint 无效".into())
}
