#![allow(non_snake_case)]

use std::collections::HashSet;
use std::io::Read;
use std::sync::Arc;

use operit_host_api::RuntimeStorageWriteHost;
use serde_json::Value;
use zip::ZipArchive;

use crate::data::archive::ArchiveSource::{ArchiveSource, ArchiveSourceReader};
use crate::data::backup::Operit1SnapshotArchive::validateRelativePath;
use crate::services::LegacyWorkflowService::LegacyWorkflowService;

const WORKFLOW_JSON_LIMIT: u64 = 16 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LegacyImportResourceLocation { Public, Internal, External, Cache }

#[derive(Clone, Debug)]
pub(crate) struct LegacyImportResourceItem {
    pub(crate) entryName: String,
    pub(crate) relativePath: String,
    pub(crate) location: LegacyImportResourceLocation,
    pub(crate) targetPath: Option<String>,
    pub(crate) workspace: bool,
}

impl LegacyImportResourceItem {
    pub(crate) fn sourcePrefixes(&self, package: &str) -> Vec<String> {
        match self.location {
            LegacyImportResourceLocation::Public => vec![
                "/storage/emulated/0/Download/Operit/".into(),
                "/sdcard/Download/Operit/".into(),
            ],
            LegacyImportResourceLocation::Internal => vec![
                format!("/data/user/0/{package}/files/"),
                format!("/data/data/{package}/files/"),
            ],
            LegacyImportResourceLocation::External => vec![
                format!("/storage/emulated/0/Android/data/{package}/files/"),
                format!("/sdcard/Android/data/{package}/files/"),
            ],
            LegacyImportResourceLocation::Cache => vec![
                format!("/data/user/0/{package}/cache/"),
                format!("/data/data/{package}/cache/"),
            ],
        }
    }
}

#[derive(Clone)]
pub(crate) struct LegacyImportResourcesArchive {
    source: Arc<dyn ArchiveSource>,
    pub(crate) items: Vec<LegacyImportResourceItem>,
    workflows: Vec<(String, Value)>,
    notes: Vec<String>,
    isZip: bool,
}

impl std::fmt::Debug for LegacyImportResourcesArchive {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LegacyImportResourcesArchive")
            .field("fileCount", &self.items.len())
            .field("workflowCount", &self.workflows.len()).finish()
    }
}

fn workflowDefinitions(value: Value) -> Result<Vec<Value>, String> {
    let values = match value {
        Value::Array(values) => values,
        Value::Object(mut value) if value.get("workflows").is_some_and(Value::is_array) =>
            value.remove("workflows").unwrap().as_array().unwrap().clone(),
        value if value.get("nodes").is_some_and(Value::is_array) => vec![value],
        _ => return Err("JSON 不是一代工作流：需要 nodes 数组、工作流数组或 workflows 数组".into()),
    };
    if values.iter().any(|value| !value.get("nodes").is_some_and(Value::is_array)) {
        return Err("工作流列表有缺少 nodes 数组的项目".into());
    }
    Ok(values)
}

fn classifyEntry(name: &str) -> Result<Option<(LegacyImportResourceLocation, String)>, String> {
    let name = name.replace('\\', "/");
    let name = name.strip_prefix("./").unwrap_or(&name);
    if name.starts_with("__MACOSX/") || name.ends_with("/.DS_Store") || name == ".DS_Store" {
        return Ok(None);
    }
    validateRelativePath(name)?;
    for (prefix, kind) in [
        ("storage/emulated/0/Download/Operit/", LegacyImportResourceLocation::Public),
        ("sdcard/Download/Operit/", LegacyImportResourceLocation::Public),
        ("Download/Operit/", LegacyImportResourceLocation::Public),
        ("Operit/", LegacyImportResourceLocation::Public),
        ("public/", LegacyImportResourceLocation::Public),
        ("payload/external_files/", LegacyImportResourceLocation::External),
        ("external_files/", LegacyImportResourceLocation::External),
        ("payload/files/", LegacyImportResourceLocation::Internal),
        ("files/", LegacyImportResourceLocation::Internal),
        ("cache/", LegacyImportResourceLocation::Cache),
    ] {
        if let Some(relative) = name.strip_prefix(prefix) {
            validateRelativePath(relative)?;
            return Ok(Some((kind, relative.to_string())));
        }
    }
    Ok(Some((LegacyImportResourceLocation::Public, name.to_string())))
}

impl LegacyImportResourcesArchive {
    /// Reads the directory only; resource payloads remain in the staged archive.
    pub(crate) fn fromSource(source: Arc<dyn ArchiveSource>) -> Result<Self, String> {
        let signature = source.readAt(0, 4)?;
        let isZip = signature.starts_with(b"PK");
        let mut result = Self { source: source.clone(), items: Vec::new(),
            workflows: Vec::new(), notes: Vec::new(), isZip };
        if !isZip {
            if source.len()? > WORKFLOW_JSON_LIMIT {
                return Err("单独工作流 JSON 超过 16 MiB，请检查是否误选了其它文件".into());
            }
            let mut bytes = Vec::new();
            ArchiveSourceReader::new(source).read_to_end(&mut bytes).map_err(|e| e.to_string())?;
            let text = std::str::from_utf8(&bytes).map_err(|e| format!("工作流不是 UTF-8 JSON：{e}"))?;
            let value = serde_json::from_str(text.trim_start_matches('\u{feff}'))
                .map_err(|e| format!("工作流 JSON 无法解析：{e}"))?;
            for value in workflowDefinitions(value)? {
                result.workflows.push(("所选 JSON".into(), value));
            }
            return Ok(result);
        }
        let mut zip = ZipArchive::new(ArchiveSourceReader::new(source)).map_err(|e| e.to_string())?;
        let mut paths = HashSet::new();
        for index in 0..zip.len() {
            let mut file = zip.by_index(index).map_err(|e| e.to_string())?;
            if file.is_dir() { continue; }
            let entryName = file.name().to_string();
            let Some((location, relativePath)) = classifyEntry(&entryName)? else { continue; };
            if !paths.insert(format!("{location:?}/{relativePath}")) {
                return Err(format!("补充资源包含两个相同目标路径：{relativePath}"));
            }
            let workflowFile = location == LegacyImportResourceLocation::Public
                && (relativePath.starts_with("workflow/") || relativePath.starts_with("workflows/"))
                && relativePath.to_ascii_lowercase().ends_with(".json")
                && !relativePath.contains("/_execution_logs/");
            if workflowFile {
                let parsed = (|| {
                    if file.size() > WORKFLOW_JSON_LIMIT { return Err("工作流 JSON 超过 16 MiB".to_string()); }
                    let mut text = String::new();
                    file.read_to_string(&mut text).map_err(|e| e.to_string())?;
                    let value = serde_json::from_str(text.trim_start_matches('\u{feff}'))
                        .map_err(|e| e.to_string())?;
                    workflowDefinitions(value)
                })();
                match parsed {
                    Ok(values) => result.workflows.extend(values.into_iter().map(|value| (relativePath.clone(), value))),
                    Err(error) => result.notes.push(format!("工作流 {relativePath} 未载入：{error}；原始 JSON 仍随资源保存")),
                }
            }
            result.items.push(LegacyImportResourceItem { entryName, relativePath, location,
                targetPath: None, workspace: false });
        }
        if result.items.is_empty() { return Err("补充 ZIP 没有可导入的文件".into()); }
        Ok(result)
    }

    /// Streams each actual file to its final import destination in bounded chunks.
    pub(crate) fn copyFiles(&self, host: &dyn RuntimeStorageWriteHost) -> Result<(), String> {
        if !self.isZip { return Ok(()); }
        let mut zip = ZipArchive::new(ArchiveSourceReader::new(self.source.clone()))
            .map_err(|e| e.to_string())?;
        let mut buffer = vec![0u8; 256 * 1024];
        for item in &self.items {
            let Some(target) = &item.targetPath else { continue; };
            let mut entry = zip.by_name(&item.entryName).map_err(|e| e.to_string())?;
            let mut session = host.createWriteSession(target).map_err(|e| e.to_string())?;
            loop {
                let count = entry.read(&mut buffer)
                    .map_err(|e| format!("读取补充资源 {} 失败：{e}", item.entryName))?;
                if count == 0 { break; }
                session.writeChunk(&buffer[..count]).map_err(|e| e.to_string())?;
            }
            session.commit().map_err(|e| format!("保存补充资源 {} 失败：{e}", item.entryName))?;
        }
        Ok(())
    }

    pub(crate) fn internalFileCount(&self) -> i32 {
        self.items.iter().filter(|item| item.targetPath.is_some() && !item.workspace
            && matches!(item.location, LegacyImportResourceLocation::Internal | LegacyImportResourceLocation::Cache)).count() as i32
    }
    pub(crate) fn externalFileCount(&self) -> i32 {
        self.items.iter().filter(|item| item.targetPath.is_some() && !item.workspace
            && matches!(item.location, LegacyImportResourceLocation::Public | LegacyImportResourceLocation::External)).count() as i32
    }
    pub(crate) fn workspaceFileCount(&self) -> i32 {
        self.items.iter().filter(|item| item.targetPath.is_some() && item.workspace).count() as i32
    }

    pub(crate) fn importWorkflows(&self, rewrite: impl Fn(&mut Value)) -> Vec<String> {
        let mut notes = self.notes.clone();
        let mut imported = 0u64;
        let mut existing = 0u64;
        for (name, workflow) in &self.workflows {
            let mut workflow = workflow.clone();
            rewrite(&mut workflow);
            // Importing a backup must not fire its alarms before the user reviews them.
            workflow["enabled"] = Value::Bool(false);
            match LegacyWorkflowService::import_definitions(vec![workflow]) {
                Ok(result) => {
                    imported += result["imported"].as_u64().unwrap_or(0);
                    existing += result["skippedExisting"].as_u64().unwrap_or(0);
                }
                Err(error) => notes.push(format!("工作流 {name} 未载入：{error}；请保留源文件")),
            }
        }
        if !self.workflows.is_empty() {
            notes.push(format!("补充工作流：已导入 {imported} 个（默认关闭，查看后可启用），已有同 ID 的 {existing} 个保留现状"));
        }
        if !self.items.is_empty() {
            notes.push(format!("补充资源：已复制 {} 个文件，聊天附件和工作区路径已按实际文件映射；未提供的原始文件不会凭空恢复",
                self.internalFileCount() + self.externalFileCount() + self.workspaceFileCount()));
        }
        notes
    }
}
