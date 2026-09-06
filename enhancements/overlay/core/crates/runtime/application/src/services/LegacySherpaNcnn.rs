//! Restores the old fixed bilingual NCNN recognizer through real LOCAL_MODEL assets.
//! The caller runs this on a host job worker and commits the returned configuration.
use operit_host_api::HostManager::HostManager;
use operit_host_api::TimeUtils::currentTimeMillis;
use operit_local_models::LocalEngineManifest::{LocalPlatform, LocalPlatformTarget};
use operit_local_models::LocalModelCatalog::LocalModelCatalog;
use operit_local_models::LocalModelManifest::LocalModelManifest;
use operit_local_models::LocalModelRegistry::InstalledLocalModel;
use operit_local_models::LocalModelStorage::buildLocalModelStoragePath;
use operit_model::SttCatalog::SttCatalog;
use operit_model::SttConfig::SttConfig;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::path::PathBuf;

use crate::data::preferences::SttConfigManager::SttConfigManager;
use crate::services::LocalModelService::LocalModelService;

pub fn prepare(host: &HostManager, request: &Value) -> Result<SttConfig, String> {
    if LocalPlatformTarget::current()?.platform != LocalPlatform::Android {
        return Err("旧 Sherpa NCNN 识别器目前由 Android 原生宿主提供".into());
    }
    if host.localInferenceHost.is_none() {
        return Err("Android NCNN 语音识别宿主未注册".into());
    }
    let manager = SttConfigManager::getInstance();
    let mut config = match manager.getSelectedSttConfigId()? {
        Some(id) => manager.getSttConfig(&id)?,
        None => {
            let provider = SttCatalog::provider("LOCAL_MODEL")?;
            let now = currentTimeMillis();
            SttConfig {
                id: uuid::Uuid::new_v4().to_string(),
                name: "Operit 1 本地语音识别".into(),
                providerType: provider.providerTypeId,
                endpoint: provider.defaultEndpoint,
                apiKey: String::new(),
                model: provider.defaultModel,
                fileFieldName: provider.defaultFileFieldName,
                modelFieldName: provider.defaultModelFieldName,
                languageFieldName: provider.defaultLanguageFieldName,
                responseTextJsonPath: provider.defaultResponseTextJsonPath,
                headers: provider.defaultHeaders,
                createdAt: now,
                updatedAt: now,
            }
        }
    };
    let manifest = LocalModelCatalog::oldAssistanceSherpaNcnnStt();
    let service = LocalModelService::getInstance(host)?;
    let storage_path = buildLocalModelStoragePath(
        &manifest.kind,
        &manifest.engine,
        &manifest.id,
        &manifest.version,
    )
    .map_err(|error| error.to_string())?;
    let mut ready = verified_local_files(host, &storage_path, &manifest)?;
    // The importer may supply the restored model directory explicitly. All seven
    // fixed files must match the original immutable model before any registration.
    if !ready {
        if let Some(directory) = request
            .get("legacy_ncnn_model_directory")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
        {
            restore_model_files(host, directory, &storage_path, &manifest)?;
            ready = verified_local_files(host, &storage_path, &manifest)?;
            if !ready {
                return Err("导入的 NCNN 模型文件验证失败".into());
            }
        }
    }
    if ready {
        let now = currentTimeMillis();
        service.registerLegacyNcnnModel(InstalledLocalModel {
            manifest: manifest.clone(),
            storagePath: storage_path,
            installedAtMs: now,
            verifiedAtMs: Some(now),
        })?;
    } else {
        // Downloads use the original file sizes/checksums and the exact NCNN archive.
        service.installModel(manifest.id.clone(), manifest.version.clone())?;
    }
    config.providerType = "LOCAL_MODEL".into();
    config.model = manifest.registryKey();
    config.endpoint.clear();
    config.apiKey.clear();
    config.headers.clear();
    Ok(config)
}

fn verified_local_files(
    host: &HostManager,
    prefix: &str,
    manifest: &LocalModelManifest,
) -> Result<bool, String> {
    let storage = host
        .runtimeStorageHost
        .as_ref()
        .ok_or("NCNN 需要运行时存储 Host")?;
    let root = storage.runtimeRootDir().ok_or("NCNN 运行时存储未初始化")?;
    let files = host
        .fileSystemHost
        .as_ref()
        .ok_or("NCNN 需要文件系统 Host")?;
    for file in &manifest.files {
        let path = format!("{prefix}/{}", file.relativePath);
        let relative = path
            .strip_prefix("runtime/")
            .ok_or("NCNN 模型存储路径无效")?;
        let info = files
            .fileExists(&root.join(relative).to_string_lossy())
            .map_err(|e| e.to_string())?;
        if !info.exists || info.isDirectory || info.size < 0 || info.size as u64 != file.byteSize {
            return Ok(false);
        }
        let mut digest = Sha256::new();
        let mut offset = 0;
        while offset < file.byteSize {
            let count = (file.byteSize - offset).min(64 * 1024) as usize;
            let bytes = storage
                .readBytesRange(&path, offset, count)
                .map_err(|e| e.to_string())?;
            if bytes.len() != count {
                return Ok(false);
            }
            digest.update(bytes);
            offset += count as u64;
        }
        if format!("{:x}", digest.finalize()) != file.sha256 {
            return Ok(false);
        }
    }
    Ok(true)
}

fn restore_model_files(
    host: &HostManager,
    directory: &str,
    destination: &str,
    manifest: &LocalModelManifest,
) -> Result<(), String> {
    let fs = host
        .fileSystemHost
        .as_ref()
        .ok_or("NCNN 需要文件系统 Host")?;
    let storage = host
        .runtimeStorageHost
        .as_ref()
        .ok_or("NCNN 需要运行时存储 Host")?;
    let root = storage.runtimeRootDir().ok_or("NCNN 运行时存储未初始化")?;
    fs.validatePath(directory, "legacy_ncnn_model_directory")
        .map_err(|e| e.to_string())?;
    let source = PathBuf::from(fs.canonicalizePath(directory).map_err(|e| e.to_string())?);
    let stage_path = format!("runtime/cache/legacy-ncnn-import-{}", uuid::Uuid::new_v4());
    let stage = root.join(
        stage_path
            .strip_prefix("runtime/")
            .ok_or("NCNN 暂存路径无效")?,
    );
    let outcome = (|| {
        fs.makeDirectory(&stage.to_string_lossy(), true)
            .map_err(|e| e.to_string())?;
        for file in &manifest.files {
            let input = PathBuf::from(
                fs.canonicalizePath(&source.join(&file.relativePath).to_string_lossy())
                    .map_err(|e| e.to_string())?,
            );
            if !input.starts_with(&source) {
                return Err("旧 NCNN 模型文件超出其目录".to_string());
            }
            let info = fs
                .fileExists(&input.to_string_lossy())
                .map_err(|e| e.to_string())?;
            if !info.exists
                || info.isDirectory
                || info.size < 0
                || info.size as u64 != file.byteSize
            {
                return Err(format!(
                    "旧 NCNN 模型文件缺失或大小不符：{}",
                    file.relativePath
                ));
            }
            fs.copyFile(
                &input.to_string_lossy(),
                &stage.join(&file.relativePath).to_string_lossy(),
                false,
            )
            .map_err(|e| e.to_string())?;
        }
        if !verified_local_files(host, &stage_path, manifest)? {
            return Err("旧 NCNN 模型的 SHA-256 与原始中英模型不一致".into());
        }
        let target = root.join(
            destination
                .strip_prefix("runtime/")
                .ok_or("NCNN 模型存储路径无效")?,
        );
        fs.makeDirectory(&target.to_string_lossy(), true)
            .map_err(|e| e.to_string())?;
        for file in &manifest.files {
            fs.copyFile(
                &stage.join(&file.relativePath).to_string_lossy(),
                &target.join(&file.relativePath).to_string_lossy(),
                false,
            )
            .map_err(|e| e.to_string())?;
        }
        Ok(())
    })();
    let cleanup = fs
        .deleteFile(&stage.to_string_lossy(), true)
        .map_err(|e| e.to_string());
    outcome.and(cleanup)
}
