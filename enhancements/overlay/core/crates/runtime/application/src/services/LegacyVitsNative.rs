//! Actual Operit 1 custom VITS ONNX support, retaining the stock Sherpa path when compatible.
use operit_host_api::HostManager::HostManager;
use operit_model::TtsConfig::TtsConfig;
use operit_providers::tts::VoiceService::VoiceService;
use serde_json::{json, Value};
use super::LegacyVitsImport;
use crate::data::preferences::TtsConfigManager::TtsConfigManager;
use operit_tools::files::PathMapper::PathMapper;

/// Prepare only; the caller decides whether to save or create the returned profile.
pub fn prepare(host: &HostManager, request: &Value) -> Result<TtsConfig, String> {
    let mut config = match LegacyVitsImport::prepare(host, request) {
        Ok(config) => config,
        Err(error) if requires_original_inference(&error) => prepare_native(host, request)
            .map_err(|native| format!("Sherpa 路径不适用：{error}\n原版 ONNX 推理初始化：{native}"))?,
        Err(error) => return Err(error),
    };
    if let Some(rate) = request.get("tts_speech_rate") {
        let rate = rate.as_f64().or_else(|| rate.as_str().and_then(|s| s.parse().ok()))
            .ok_or("tts_speech_rate 必须是有效数字")?;
        if !rate.is_finite() || rate <= 0.0 { return Err("tts_speech_rate 必须是有限正数".into()); }
        config.speed = rate;
    }
    if config.providerType == "LEGACY_VITS" {
        let model: Value = serde_json::from_str(&config.model).map_err(|e| e.to_string())?;
        if model["supportsSpeed"] == false && config.speed != 1.0 {
            return Err("此 VITS 模型没有 scales 输入，无法应用非 1.0 的语速；原配置保持不变".into());
        }
    }
    Ok(config)
}

// Only explicit incompatibility results from the separate Sherpa importer are routed here.
// Damaged archives, missing/ambiguous files, permissions, changing content and install failures
// are not reinterpreted as model compatibility problems.
fn requires_original_inference(error: &str) -> bool {
    ["需要自定义 ONNX", "此旧 VITS 前端需要自定义分词", "旧 ONNX 缺少 Sherpa 必需元数据",
     "此 ONNX 模型不是 Sherpa VITS 模型", "此 VITS 包还需要专用分词/字典驱动", "旧 VITS 包声明了当前驱动无法等价执行的 frontend",
     "此 VITS 包需要非默认", "此旧 VITS token 对应多个 ID", "VITS token 无法用 Sherpa tokens.txt 表示"]
        .iter().any(|marker| error.contains(marker))
}

fn prepare_native(host: &HostManager, request: &Value) -> Result<TtsConfig, String> {
    if !cfg!(target_os = "android") { return Err("自定义旧版 VITS ONNX 推理需要 Android Core".into()); }
    let package = request["tts_vits_package_path"].as_str().unwrap_or("").trim();
    if package.is_empty() { return Err("VITS 模型包路径为空".into()); }
    let path = if package.starts_with("file:") {
        let url = url::Url::parse(package).map_err(|e| e.to_string())?;
        #[cfg(not(target_arch = "wasm32"))]
        { url.to_file_path().map_err(|_| "VITS file URL 无法转换成本地路径")?.to_string_lossy().to_string() }
        #[cfg(target_arch = "wasm32")]
        { let _ = url; return Err("自定义旧版 VITS 需要原生 Android Core".into()); }
    } else { package.to_string() };
    let storage = host.runtimeStorageHost.as_ref().ok_or("VITS 需要运行时存储")?;
    let path = if path.starts_with("/app/") || path.starts_with("/mnt/") {
        PathMapper::new(storage.runtimeRootDir().ok_or("VITS 运行时目录未配置")?,
            storage.workspaceRootDir().ok_or("VITS 工作区目录未配置")?).resolve(&path)?.physicalPath
    } else { path };
    let path = host.fileSystemHost.as_ref().ok_or("VITS 需要文件系统 Host")?
        .canonicalizePath(&path).map_err(|e| e.to_string())?;
    let mut options = match request.get("tts_vits_options") {
        None | Some(Value::Null) => json!({}),
        Some(Value::String(text)) if text.trim().is_empty() => json!({}),
        Some(Value::String(text)) => serde_json::from_str(text).map_err(|e| e.to_string())?,
        Some(value) => value.clone(),
    };
    let options = options.as_object_mut().ok_or("tts_vits_options 必须是 JSON 对象")?;
    for (key, value) in options.iter_mut() {
        if !value.is_string() {
            if matches!(key.as_str(), "sample_rate" | "speaker_count") && value.is_number() { *value = Value::String(value.to_string()); }
            else { return Err(format!("旧 VITS 选项 {key} 必须是字符串")); }
        }
    }
    let speaker = match request.get("tts_vits_speaker_id") {
        None | Some(Value::Null) => "",
        Some(Value::String(value)) => value.trim(),
        _ => return Err("tts_vits_speaker_id 必须是字符串".into()),
    };
    let mut model = json!({"packagePath":path,"speakerId":speaker,"options":options});
    let probe = invoke_java("inspect", &model)?;
    // ZIP models remain usable after the original ZIP is removed. The host verified and
    // completely materialized this package before returning its persistent packageRoot.
    model["packagePath"] = Value::String(probe["packageRoot"].as_str().ok_or("VITS host did not report its package root")?.into());
    model["supportsSpeed"] = probe["supportsSpeed"].clone();
    let mut config = TtsConfigManager::getInstance().getCurrentTtsConfig()?;
    config.providerType = "LEGACY_VITS".into();
    config.model = model.to_string();
    config.voice = speaker.into();
    config.responseFormat = "wav".into();
    config.endpoint.clear(); config.apiKey.clear(); config.headers.clear();
    config.requestBody.clear(); config.responsePipeline.clear();
    Ok(config)
}

/// Uses existing application Android/JVM dispatch, with handles released after each request.
fn invoke_java(method: &str, request: &Value) -> Result<Value, String> {
    #[cfg(all(feature = "javascript", target_os = "android"))]
    {
        use operit_js_bridge::javascript::JsJavaBridgeDelegates::LegacyJavaScope;
        let scope = LegacyJavaScope::new();
        let dispatch = |request: Value| -> Result<Value, String> {
            let response: Value = serde_json::from_str(&scope.dispatch(request.to_string())).map_err(|e| e.to_string())?;
            if response["success"] != true { return Err(response["message"].as_str().unwrap_or("VITS Java call failed").into()); }
            Ok(response["data"].clone())
        };
        let context = dispatch(json!({"operation":"context"}))?;
        let output = dispatch(json!({"operation":"callStatic","className":"app.operit.LegacyVitsOnnxBridge",
            "member":method,"args":[context,request.to_string()]}))?;
        return serde_json::from_str(output.as_str().ok_or("VITS Java host returned invalid JSON")?).map_err(|e| e.to_string());
    }
    #[cfg(not(all(feature = "javascript", target_os = "android")))]
    { let _ = (method, request); Err("自定义旧版 VITS ONNX 推理需要 Android Java 宿主".into()) }
}

pub struct LegacyVitsVoiceProvider { host: HostManager }
impl LegacyVitsVoiceProvider {
    pub fn new(host: &HostManager) -> Self { Self { host: host.clone() } }
}
impl VoiceService for LegacyVitsVoiceProvider {
    fn synthesize(&self, config: &TtsConfig, text: &str) -> Result<Vec<u8>, String> {
        let mut model: Value = serde_json::from_str(&config.model).map_err(|e| format!("VITS 配置无效：{e}"))?;
        model["speakerId"] = Value::String(config.voice.clone());
        let fs = self.host.fileSystemHost.as_ref().ok_or("VITS 需要文件系统 Host")?;
        let root = self.host.runtimeStorageHost.as_ref().and_then(|s| s.runtimeRootDir()).ok_or("VITS 运行时目录未配置")?;
        let directory = root.join("data/temp/legacy-vits");
        fs.makeDirectory(&directory.to_string_lossy(), true).map_err(|e| e.to_string())?;
        let output = directory.join(format!("{}.wav", uuid::Uuid::new_v4())).to_string_lossy().to_string();
        let generated = invoke_java("synthesize", &json!({"config":model,"text":text,"speed":config.speed,"outputPath":output}));
        let result = generated.and_then(|result| {
            if result["outputFormat"] != "wav" || result["audioPath"].as_str() != Some(output.as_str()) {
                return Err("VITS host returned a different output file or format".into());
            }
            fs.readFileBytes(&output).map_err(|e| e.to_string())
        });
        let _ = fs.deleteFile(&output, false);
        result
    }
}
