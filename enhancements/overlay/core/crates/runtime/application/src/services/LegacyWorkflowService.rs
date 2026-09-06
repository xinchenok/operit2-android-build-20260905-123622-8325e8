#![allow(non_snake_case)]

use crate::plugins::toolpkg::ToolPkgHookBridgeSupport::ToolPkgBridgeRuntime;
use crate::plugins::toolpkg::ToolPkgHostEventHookBridge::syncHostEventSchedules;
use chrono::{Datelike, Duration, Local, NaiveDateTime, TimeZone, Timelike};
use operit_host_api::{
    HostRuntimeEventSchedule, HostRuntimeEventScheduleFire, HostRuntimeEventScheduleKind,
};
use operit_tools::tools::ToolResultDataClasses::ToolResultData;
use operit_tools::ToolExecutionManager::{AITool, ToolParameter};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Mutex, OnceLock};
use uuid::Uuid;

const PATH: &str = "runtime/data/legacy_workflows/state.json";
const CONTAINER: &str = "operit.enhanced.legacy-workflow";
static STORE_LOCK: Mutex<()> = Mutex::new(());
static RUNNING: OnceLock<Mutex<BTreeSet<String>>> = OnceLock::new();
static RUNTIME: OnceLock<Mutex<Option<ToolPkgBridgeRuntime>>> = OnceLock::new();

pub struct LegacyWorkflowService;

fn now() -> i64 {
    operit_host_api::TimeUtils::currentTimeMillis()
}
fn text(v: &Value) -> String {
    match v {
        Value::Null => String::new(),
        Value::String(s) => s.clone(),
        _ => v.to_string(),
    }
}
fn field(v: &Value, key: &str) -> String {
    text(&v[key])
}
fn flag(v: &Value, default: bool) -> bool {
    v.as_bool()
        .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
        .unwrap_or(default)
}
fn number(v: &Value, default: i64) -> i64 {
    v.as_i64()
        .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
        .unwrap_or(default)
}
fn array(v: &Value) -> Result<Vec<Value>, String> {
    if v.is_null() {
        return Ok(Vec::new());
    }
    let v = if let Some(s) = v.as_str() {
        serde_json::from_str(s).map_err(|e| format!("Invalid workflow JSON: {e}"))?
    } else {
        v.clone()
    };
    v.as_array()
        .cloned()
        .ok_or_else(|| "Workflow nodes/connections/patches must be arrays".to_string())
}
fn load(runtime: &ToolPkgBridgeRuntime) -> Result<Value, String> {
    let host = runtime
        .host_manager()
        .runtimeStorageHost
        .ok_or("Runtime storage is unavailable")?;
    if !host.exists(PATH).map_err(|e| e.to_string())? {
        return Ok(json!({"workflows":[],"runs":{}}));
    }
    let bytes = host.readBytes(PATH).map_err(|e| e.to_string())?;
    let value: Value =
        serde_json::from_slice(&bytes).map_err(|e| format!("Cannot read saved workflows: {e}"))?;
    if !value["workflows"].is_array() {
        return Err("Saved workflows have no workflow list".into());
    }
    Ok(value)
}
fn save(runtime: &ToolPkgBridgeRuntime, value: &Value) -> Result<(), String> {
    let context = runtime.host_manager();
    let bytes = serde_json::to_vec(value).map_err(|e| e.to_string())?;
    if let Some(host) = context.runtimeStorageWriteHost {
        let mut session = host.createWriteSession(PATH).map_err(|e| e.to_string())?;
        session.writeChunk(&bytes).map_err(|e| e.to_string())?;
        session.commit().map_err(|e| e.to_string())
    } else {
        context
            .runtimeStorageHost
            .ok_or("Runtime storage is unavailable")?
            .writeBytes(PATH, &bytes)
            .map_err(|e| e.to_string())
    }
}
fn mutate<T>(
    runtime: &ToolPkgBridgeRuntime,
    change: impl FnOnce(&mut Value) -> Result<T, String>,
) -> Result<T, String> {
    let _guard = STORE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut store = load(runtime)?;
    let previous = store.clone();
    let result = change(&mut store)?;
    if store != previous {
        save(runtime, &store)?;
    }
    Ok(result)
}
fn detail(value: &Value) -> Value {
    let mut value = value.clone();
    if let Some(object) = value.as_object_mut() {
        object.retain(|key, _| !key.starts_with('_'));
    }
    value
}
fn normalized_nodes(value: &Value) -> Result<Vec<Value>, String> {
    let mut nodes = array(value)?;
    let mut ids = BTreeSet::new();
    for node in &mut nodes {
        if !node.is_object() {
            return Err("Workflow node must be an object".into());
        }
        if field(node, "id").is_empty() {
            node["id"] = json!(Uuid::new_v4().to_string());
        }
        if !ids.insert(field(node, "id")) {
            return Err(format!("Duplicate workflow node id: {}", node["id"]));
        }
        if field(node, "type").is_empty() {
            let discriminator = field(node, "__type");
            let kind = if discriminator.ends_with("TriggerNode") || !node["triggerType"].is_null() {
                "trigger"
            } else if discriminator.ends_with("ExecuteNode") || !node["actionType"].is_null() {
                "execute"
            } else if discriminator.ends_with("ConditionNode") || !node["left"].is_null() {
                "condition"
            } else if discriminator.ends_with("ExtractNode")
                || !node["source"].is_null()
                || !node["mode"].is_null()
            {
                "extract"
            } else {
                "logic"
            };
            node["type"] = json!(kind);
        }
        for key in ["name", "description"] {
            if node[key].is_null() {
                node[key] = json!("");
            }
        }
        if node["position"].is_null() {
            node["position"] = json!({"x":0,"y":0});
        }
        match field(node, "type").as_str() {
            "trigger" => {
                if node["triggerType"].is_null() {
                    node["triggerType"] = json!("manual");
                }
                if node["triggerConfig"].is_null() {
                    node["triggerConfig"] = json!({});
                }
            }
            "execute" => {
                if node["actionConfig"].is_null() {
                    node["actionConfig"] = json!({});
                }
            }
            "condition" => {
                node["operator"] = json!(field(node, "operator").to_uppercase());
            }
            "logic" => {
                if node["operator"].is_null() {
                    node["operator"] = node["operatorLogic"].clone();
                }
            }
            "extract" => {
                if node["expression"].is_null() {
                    node["expression"] = node
                        .get("pattern")
                        .or_else(|| node.get("path"))
                        .cloned()
                        .unwrap_or(json!(""));
                }
            }
            other => return Err(format!("Unknown workflow node type: {other}")),
        }
    }
    Ok(nodes)
}
fn normalized_connections(value: &Value, nodes: &[Value]) -> Result<Vec<Value>, String> {
    let mut connections = array(value)?;
    for edge in &mut connections {
        if !edge.is_object() {
            return Err("Workflow connection must be an object".into());
        }
        if field(edge, "id").is_empty() {
            edge["id"] = json!(Uuid::new_v4().to_string());
        }
        for (key, aliases) in [
            ("sourceNodeId", ["sourceId", "source", "from"]),
            ("targetNodeId", ["targetId", "target", "to"]),
        ] {
            let raw = if !edge[key].is_null() {
                edge[key].clone()
            } else {
                aliases
                    .iter()
                    .find_map(|alias| edge.get(*alias))
                    .cloned()
                    .unwrap_or(Value::Null)
            };
            let raw_text = text(&raw);
            let id = nodes
                .iter()
                .find(|n| field(n, "id") == raw_text)
                .map(|n| n["id"].clone())
                .or_else(|| {
                    raw.as_u64()
                        .and_then(|idx| nodes.get(idx as usize))
                        .map(|n| n["id"].clone())
                })
                .or_else(|| {
                    nodes
                        .iter()
                        .find(|n| field(n, "name") == raw_text)
                        .map(|n| n["id"].clone())
                })
                .unwrap_or(raw);
            edge[key] = id;
        }
    }
    connections.retain(|edge| {
        edge["sourceNodeId"] != edge["targetNodeId"]
            && ["sourceNodeId", "targetNodeId"]
                .iter()
                .all(|key| nodes.iter().any(|node| node["id"] == edge[*key]))
    });
    Ok(connections)
}
fn normalize_workflow(workflow: &mut Value, reset_schedule: bool) -> Result<(), String> {
    let nodes = normalized_nodes(&workflow["nodes"])?;
    let edges = normalized_connections(&workflow["connections"], &nodes)?;
    workflow["nodes"] = json!(nodes);
    workflow["connections"] = json!(edges);
    workflow["updatedAt"] = json!(now());
    if reset_schedule {
        workflow["_nextRuns"] = json!({});
        workflow["_completedTimers"] = json!({});
    }
    if workflow["_nextRuns"].is_null() {
        workflow["_nextRuns"] = json!({});
    }
    if workflow["_completedTimers"].is_null() {
        workflow["_completedTimers"] = json!({});
    }
    if flag(&workflow["enabled"], true) {
        for node in nodes.iter().filter(|n| {
            field(n, "type") == "trigger"
                && field(n, "triggerType") == "schedule"
                && flag(&n["triggerConfig"]["enabled"], true)
        }) {
            let id = field(node, "id");
            if workflow["_nextRuns"][&id].is_null()
                && !flag(&workflow["_completedTimers"][&id], false)
            {
                workflow["_nextRuns"][&id] = json!(next_time(&node["triggerConfig"], now(), true)?);
            }
        }
    }
    Ok(())
}
impl LegacyWorkflowService {
    pub fn command(
        runtime: &ToolPkgBridgeRuntime,
        action: &str,
        payload: Value,
    ) -> Result<Value, String> {
        if action == "event" {
            Self::event(runtime, &payload);
            return Ok(json!("Workflow event delivered"));
        }
        if action == "trigger" {
            let id = field(&payload, "workflow_id");
            let executionId = Self::start(
                runtime,
                &id,
                payload
                    .get("trigger_node_id")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                payload.get("extras").cloned().unwrap_or(json!({})),
            )?;
            return Ok(json!({"workflowId":id,"executionId":executionId,"status":"RUNNING"}));
        }
        if action == "getAll" || action == "get" || action == "events" || action == "execution" {
            let _guard = STORE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
            let store = load(runtime)?;
            if action == "execution" {
                let id = field(&payload, "workflow_id");
                let runId = field(&payload, "execution_id");
                return store["runs"][&id]
                    .as_array()
                    .and_then(|runs| runs.iter().find(|run| field(run, "executionId") == runId))
                    .cloned()
                    .ok_or_else(|| format!("Workflow execution not found: {runId}"));
            }
            let workflows = store["workflows"]
                .as_array()
                .ok_or("Missing workflow list")?;
            if action == "events" {
                let mut actions = BTreeSet::new();
                for workflow in workflows.iter().filter(|w| flag(&w["enabled"], true)) {
                    for node in array(&workflow["nodes"])? {
                        if field(&node, "type") == "trigger"
                            && field(&node, "triggerType") == "intent"
                            && flag(&node["triggerConfig"]["enabled"], true)
                        {
                            let action = field(&node["triggerConfig"], "action");
                            if !action.is_empty() {
                                actions.insert(action);
                            }
                        }
                    }
                }
                return Ok(json!({"actions": actions}));
            }
            if action == "get" {
                let id = field(&payload, "workflow_id");
                let mut value = detail(
                    workflows
                        .iter()
                        .find(|w| field(w, "id") == id)
                        .ok_or_else(|| format!("Workflow not found: {id}"))?,
                );
                value["executionLogs"] = store["runs"][&id].clone();
                return Ok(value);
            }
            let summary = workflows
                .iter()
                .map(|w| {
                    let mut v = detail(w);
                    v["nodeCount"] = json!(w["nodes"].as_array().map_or(0, Vec::len));
                    v["connectionCount"] = json!(w["connections"].as_array().map_or(0, Vec::len));
                    v.as_object_mut().unwrap().remove("nodes");
                    v.as_object_mut().unwrap().remove("connections");
                    v
                })
                .collect::<Vec<_>>();
            return Ok(json!({"totalCount":summary.len(),"workflows":summary}));
        }
        let result = mutate(runtime, |store| {
            let list = store["workflows"]
                .as_array_mut()
                .ok_or("Missing workflow list")?;
            if action == "import" {
                let mut imported = 0;
                let mut skipped = 0;
                for mut value in array(payload.get("workflows").unwrap_or(&payload))? {
                    if !value.is_object() {
                        return Err("Imported workflow must be an object".into());
                    }
                    if field(&value, "id").is_empty() {
                        value["id"] = json!(Uuid::new_v4().to_string());
                    }
                    if list.iter().any(|existing| existing["id"] == value["id"]) {
                        skipped += 1;
                        continue;
                    }
                    for (key, default) in [
                        ("name", json!("Imported workflow")),
                        ("description", json!("")),
                        ("enabled", json!(true)),
                        ("createdAt", json!(now())),
                        ("lastExecutionTime", Value::Null),
                        ("lastExecutionStatus", Value::Null),
                        ("totalExecutions", json!(0)),
                        ("successfulExecutions", json!(0)),
                        ("failedExecutions", json!(0)),
                    ] {
                        if value[key].is_null() {
                            value[key] = default;
                        }
                    }
                    normalize_workflow(&mut value, true)?;
                    list.push(value);
                    imported += 1;
                }
                return Ok(json!({"imported":imported,"skippedExisting":skipped}));
            }
            if action == "create" {
                let stamp = now();
                let mut value = json!({"id":Uuid::new_v4().to_string(),"name":field(&payload,"name"),"description":field(&payload,"description"),"enabled":flag(&payload["enabled"],true),"nodes":payload["nodes"],"connections":payload["connections"],"createdAt":stamp,"updatedAt":stamp,"lastExecutionTime":null,"lastExecutionStatus":null,"totalExecutions":0,"successfulExecutions":0,"failedExecutions":0});
                normalize_workflow(&mut value, true)?;
                let result = detail(&value);
                list.push(value);
                return Ok(result);
            }
            let id = field(&payload, "workflow_id");
            let index = list
                .iter()
                .position(|w| field(w, "id") == id)
                .ok_or_else(|| format!("Workflow not found: {id}"))?;
            if action == "delete" {
                list.remove(index);
                return Ok(json!(format!("Workflow deleted: {id}")));
            }
            let workflow = &mut list[index];
            match action {
                "enable" => workflow["enabled"] = json!(true),
                "disable" => workflow["enabled"] = json!(false),
                "update" | "patch" => {
                    for key in ["name", "description", "enabled", "nodes", "connections"] {
                        if let Some(value) = payload.get(key) {
                            workflow[key] = value.clone();
                        }
                    }
                    if action == "patch" {
                        patch_items(workflow, "nodes", &payload["node_patches"], "node")?;
                        patch_items(
                            workflow,
                            "connections",
                            &payload["connection_patches"],
                            "connection",
                        )?;
                    }
                }
                other => return Err(format!("Unknown legacy-workflow action: {other}")),
            }
            normalize_workflow(
                workflow,
                payload.get("nodes").is_some() || payload.get("node_patches").is_some(),
            )?;
            Ok(detail(workflow))
        })?;
        syncHostEventSchedules(runtime);
        Ok(result)
    }
    pub fn import_definitions(definitions: Vec<Value>) -> Result<Value, String> {
        let runtime = RUNTIME
            .get_or_init(|| Mutex::new(None))
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
            .ok_or("Workflow runtime has not initialized")?;
        Self::command(&runtime, "import", json!({"workflows":definitions}))
    }
    pub fn initialize(runtime: &ToolPkgBridgeRuntime) {
        *RUNTIME
            .get_or_init(|| Mutex::new(None))
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = Some(runtime.clone());
        let result = mutate(runtime, |store| {
            for workflow in store["workflows"]
                .as_array_mut()
                .ok_or("Missing workflow list")?
            {
                if field(workflow, "lastExecutionStatus") == "RUNNING" {
                    workflow["lastExecutionStatus"] = json!("FAILED");
                    workflow["lastExecutionError"]=json!("Application stopped before this execution completed; not resent automatically");
                    workflow["failedExecutions"] =
                        json!(number(&workflow["failedExecutions"], 0) + 1);
                }
                normalize_workflow(workflow, false)?;
            }
            if let Some(runs) = store["runs"].as_object_mut() {
                for entries in runs.values_mut() {
                    if let Some(entries) = entries.as_array_mut() {
                        for run in entries {
                            if field(run, "status") == "RUNNING" {
                                run["status"] = json!("FAILED");
                                run["error"] =
                                    json!("Application stopped before execution completed");
                                run["finishedAt"] = json!(now());
                            }
                        }
                    }
                }
            }
            Ok(())
        });
        if let Err(error) = result {
            log_error(&error);
        }
        syncHostEventSchedules(runtime);
        Self::event(
            runtime,
            &json!({"topic":"legacy.workflow.app_open","data":{}}),
        );
    }
    pub fn speech(text: &str, is_final: bool) {
        let runtime = RUNTIME
            .get_or_init(|| Mutex::new(None))
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        if let Some(runtime) = runtime {
            Self::event(
                &runtime,
                &json!({"topic":"legacy.workflow.speech","data":{"text":text,"is_final":is_final}}),
            );
        }
    }
    pub fn schedules(runtime: &ToolPkgBridgeRuntime) -> Vec<HostRuntimeEventSchedule> {
        let _guard = STORE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let store = match load(runtime) {
            Ok(v) => v,
            Err(error) => {
                log_error(&error);
                return Vec::new();
            }
        };
        let mut output = Vec::new();
        for workflow in store["workflows"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|w| flag(&w["enabled"], true))
        {
            if let Some(nexts) = workflow["_nextRuns"].as_object() {
                for (node_id, next) in nexts {
                    let Some(next) = next.as_i64() else {
                        continue;
                    };
                    let schedule_id = serde_json::to_string(&[
                        CONTAINER,
                        &field(workflow, "id"),
                        node_id,
                        &next.to_string(),
                    ])
                    .unwrap_or_default();
                    output.push(HostRuntimeEventSchedule {
                        scheduleId: schedule_id,
                        containerPackageName: CONTAINER.into(),
                        hookId: node_id.clone(),
                        kind: HostRuntimeEventScheduleKind::Timer,
                        delayMs: (next - now()).max(1) as u64,
                        intervalMs: None,
                    });
                }
            }
        }
        output
    }
    pub fn scheduled(runtime: &ToolPkgBridgeRuntime, fire: &HostRuntimeEventScheduleFire) -> bool {
        let Ok(parts) = serde_json::from_str::<Vec<String>>(&fire.scheduleId) else {
            return false;
        };
        if parts.len() != 4 || parts[0] != CONTAINER {
            return false;
        }
        let id = &parts[1];
        let trigger = &parts[2];
        let accepted = mutate(runtime, |store| {
            let Some(workflow) = store["workflows"]
                .as_array_mut()
                .ok_or("Missing workflow list")?
                .iter_mut()
                .find(|w| field(w, "id") == *id)
            else {
                return Ok(false);
            };
            if !flag(&workflow["enabled"], true)
                || workflow["_nextRuns"][trigger].is_null()
                || number(&workflow["_nextRuns"][trigger], 0).to_string() != parts[3]
            {
                return Ok(false);
            }
            let nodes = array(&workflow["nodes"])?;
            let Some(node) = nodes.iter().find(|n| field(n, "id") == *trigger) else {
                return Ok(false);
            };
            let config = &node["triggerConfig"];
            if field(config, "schedule_type") == "interval" || flag(&config["repeat"], false) {
                workflow["_nextRuns"][trigger] = json!(next_time(config, now(), false)?);
            } else {
                workflow["_nextRuns"]
                    .as_object_mut()
                    .unwrap()
                    .remove(trigger);
                workflow["_completedTimers"][trigger] = json!(true);
            }
            Ok(true)
        });
        match accepted {
            Ok(true) => {
                if let Err(error) = Self::start(
                    runtime,
                    id,
                    Some(trigger.clone()),
                    json!({"scheduledAtMillis":fire.scheduledAtMillis,"firedAtMillis":fire.firedAtMillis}),
                ) {
                    log_error(&error);
                }
            }
            Ok(false) => {}
            Err(error) => log_error(&error),
        }
        syncHostEventSchedules(runtime);
        true
    }
    pub fn event(runtime: &ToolPkgBridgeRuntime, event: &Value) {
        let topic = field(event, "topic");
        let data = event
            .get("data")
            .or_else(|| event.get("payload"))
            .cloned()
            .unwrap_or(event.clone());
        let matches = mutate(runtime, |store| {
            let mut matches = Vec::new();
            for workflow in store["workflows"]
                .as_array_mut()
                .ok_or("Missing workflow list")?
                .iter_mut()
                .filter(|w| flag(&w["enabled"], true))
            {
                for node in array(&workflow["nodes"])? {
                    if field(&node, "type") != "trigger"
                        || !flag(&node["triggerConfig"]["enabled"], true)
                    {
                        continue;
                    }
                    let config = &node["triggerConfig"];
                    let kind = field(&node, "triggerType");
                    let matched = match kind.as_str() {
                        "app_open" => topic == "legacy.workflow.app_open",
                        "intent" => {
                            (topic == "legacy.workflow.intent"
                                || field(event, "source") == "android.broadcast")
                                && (field(config, "action") == field(&data, "action")
                                    || field(config, "action") == topic)
                        }
                        "tasker" => {
                            topic == "legacy.workflow.tasker"
                                && text(&data).contains(&field(config, "command"))
                        }
                        "speech" if topic == "legacy.workflow.speech" => {
                            if flag(&config["require_final"], true)
                                && !flag(
                                    data.get("is_final")
                                        .or_else(|| data.get("isFinal"))
                                        .unwrap_or(&Value::Null),
                                    true,
                                )
                            {
                                false
                            } else {
                                regex::RegexBuilder::new(&field(config, "pattern"))
                                    .case_insensitive(flag(&config["ignore_case"], true))
                                    .build()
                                    .map(|regex| regex.is_match(&field(&data, "text")))
                                    .unwrap_or(false)
                            }
                        }
                        _ => false,
                    };
                    if !matched {
                        continue;
                    }
                    let node_id = field(&node, "id");
                    if kind == "speech" {
                        let previous = number(&workflow["_lastSpeech"][&node_id], 0);
                        if now() - previous < number(&config["cooldown_ms"], 3000).max(0) {
                            continue;
                        }
                        if workflow["_lastSpeech"].is_null() {
                            workflow["_lastSpeech"] = json!({});
                        }
                        workflow["_lastSpeech"][&node_id] = json!(now());
                    }
                    matches.push((field(workflow, "id"), node_id));
                }
            }
            Ok(matches)
        });
        match matches {
            Ok(items) => {
                for (id, node) in items {
                    if let Err(error) = Self::start(runtime, &id, Some(node), data.clone()) {
                        log_error(&error);
                    }
                }
            }
            Err(error) => log_error(&error),
        }
    }
    fn start(
        runtime: &ToolPkgBridgeRuntime,
        id: &str,
        trigger: Option<String>,
        extras: Value,
    ) -> Result<String, String> {
        let mut active = RUNNING
            .get_or_init(|| Mutex::new(BTreeSet::new()))
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if !active.insert(id.to_string()) {
            return Err(format!("Workflow is already running: {id}"));
        }
        drop(active);
        let executionId = Uuid::new_v4().to_string();
        let prepared = mutate(runtime, |store| {
            let workflow = store["workflows"]
                .as_array_mut()
                .ok_or("Missing workflow list")?
                .iter_mut()
                .find(|w| field(w, "id") == id)
                .ok_or_else(|| format!("Workflow not found: {id}"))?;
            if !flag(&workflow["enabled"], true) {
                return Err("Workflow is disabled".into());
            }
            workflow["lastExecutionTime"] = json!(now());
            workflow["lastExecutionStatus"] = json!("RUNNING");
            workflow["totalExecutions"] = json!(number(&workflow["totalExecutions"], 0) + 1);
            let snapshot = workflow.clone();
            if store["runs"].is_null() {
                store["runs"] = json!({});
            }
            if store["runs"][id].is_null() {
                store["runs"][id] = json!([]);
            }
            store["runs"][id].as_array_mut().ok_or("Invalid workflow runs")?.push(json!({"executionId":executionId,"workflowId":id,"status":"RUNNING","startedAt":now()}));
            Ok(snapshot)
        });
        let workflow = match prepared {
            Ok(value) => value,
            Err(error) => {
                clear_running(id);
                return Err(error);
            }
        };
        let host = runtime.host_manager();
        let runtime = runtime.clone();
        let workflow_id = id.to_string();
        let scheduler = match host.hostRuntimeTaskSchedulerHost {
            Some(value) => value,
            None => {
                clear_running(id);
                finish(
                    &runtime,
                    id,
                    &executionId,
                    false,
                    json!({}),
                    Some("Runtime task scheduler is unavailable".into()),
                )?;
                return Err("Runtime task scheduler is unavailable".into());
            }
        };
        let task_runtime = runtime.clone();
        let task_id = workflow_id.clone();
        let task_run_id = executionId.clone();
        let result = scheduler.scheduleHostRuntimeTask(
            "legacy-workflow.execute",
            Box::new(move || {
                let execution = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    execute(&task_runtime, &workflow, trigger, extras)
                }));
                let (success, results, error) = match execution {
                    Ok(Ok((success, results))) => (success, results, None),
                    Ok(Err(error)) => (false, json!({}), Some(error)),
                    Err(_) => (
                        false,
                        json!({}),
                        Some("Workflow execution interrupted by a runtime exception".into()),
                    ),
                };
                if let Err(error) = finish(
                    &task_runtime,
                    &task_id,
                    &task_run_id,
                    success,
                    results,
                    error,
                ) {
                    log_error(&error);
                }
                clear_running(&task_id);
                syncHostEventSchedules(&task_runtime);
            }),
        );
        if let Err(error) = result {
            clear_running(id);
            finish(
                &runtime,
                id,
                &executionId,
                false,
                json!({}),
                Some(error.to_string()),
            )?;
            return Err(error.to_string());
        }
        Ok(executionId)
    }
}
fn clear_running(id: &str) {
    RUNNING
        .get_or_init(|| Mutex::new(BTreeSet::new()))
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .remove(id);
}
fn finish(
    runtime: &ToolPkgBridgeRuntime,
    id: &str,
    executionId: &str,
    success: bool,
    results: Value,
    error: Option<String>,
) -> Result<(), String> {
    mutate(runtime, |store| {
        let status = if success { "SUCCESS" } else { "FAILED" };
        // The final action may intentionally delete its own definition. Never recreate it.
        if let Some(workflow) = store["workflows"]
            .as_array_mut()
            .ok_or("Missing workflow list")?
            .iter_mut()
            .find(|w| field(w, "id") == id)
        {
            workflow["lastExecutionStatus"] = json!(status);
            workflow["lastExecutionError"] = json!(error);
            let counter = if success {
                "successfulExecutions"
            } else {
                "failedExecutions"
            };
            workflow[counter] = json!(number(&workflow[counter], 0) + 1);
        }
        if store["runs"].is_null() {
            store["runs"] = json!({});
        }
        if store["runs"][id].is_null() {
            store["runs"][id] = json!([]);
        }
        let logs = store["runs"][id]
            .as_array_mut()
            .ok_or("Invalid execution logs")?;
        if let Some(run) = logs
            .iter_mut()
            .find(|run| field(run, "executionId") == executionId)
        {
            run["finishedAt"] = json!(now());
            run["status"] = json!(status);
            run["nodes"] = results;
            run["error"] = json!(error);
        }
        if logs.len() > 20 {
            logs.remove(0);
        }
        Ok(())
    })
}
fn patch_items(
    workflow: &mut Value,
    key: &str,
    patches: &Value,
    item_key: &str,
) -> Result<(), String> {
    let mut items = array(&workflow[key])?;
    for patch in array(patches)? {
        let item = patch[item_key].clone();
        let id = patch
            .get("id")
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| field(&item, "id"));
        match field(&patch, "op").as_str() {
            "add" => items.push(item),
            "remove" => items.retain(|v| field(v, "id") != id),
            "update" => {
                let current = items
                    .iter_mut()
                    .find(|v| field(v, "id") == id)
                    .ok_or_else(|| format!("Cannot update missing {item_key}: {id}"))?;
                for (k, v) in item.as_object().ok_or("Patch item must be an object")? {
                    current[k] = v.clone();
                }
            }
            other => return Err(format!("Unknown workflow patch operation: {other}")),
        }
    }
    workflow[key] = json!(items);
    Ok(())
}
fn log_error(error: &str) {
    operit_util::AppLogger::AppLogger::e("LegacyWorkflow", error);
}

fn next_time(config: &Value, after: i64, first: bool) -> Result<i64, String> {
    match field(config, "schedule_type").as_str() {
        "interval" => {
            let duration = number(&config["interval_ms"], 0);
            if duration <= 0 {
                return Err("Workflow interval_ms must be positive".into());
            }
            Ok(after.saturating_add(duration))
        }
        "specific_time" => {
            let raw = field(config, "specific_time");
            let stamp = if let Ok(value) = raw.parse::<i64>() {
                if value < 100_000_000_000 {
                    value.saturating_mul(1000)
                } else {
                    value
                }
            } else if let Ok(value) = chrono::DateTime::parse_from_rfc3339(&raw) {
                value.timestamp_millis()
            } else {
                let parsed = [
                    "%Y-%m-%d %H:%M:%S",
                    "%Y-%m-%d %H:%M",
                    "%Y/%m/%d %H:%M:%S",
                    "%Y/%m/%d %H:%M",
                    "%Y-%m-%dT%H:%M:%S",
                ]
                .iter()
                .find_map(|format| NaiveDateTime::parse_from_str(&raw, format).ok())
                .ok_or_else(|| format!("Invalid workflow specific_time: {raw}"))?;
                Local
                    .from_local_datetime(&parsed)
                    .earliest()
                    .ok_or("The scheduled local time does not exist")?
                    .timestamp_millis()
            };
            if first && stamp > after {
                return Ok(stamp);
            }
            if flag(&config["repeat"], false) {
                let base = Local
                    .timestamp_millis_opt(stamp)
                    .single()
                    .ok_or("Invalid schedule timestamp")?;
                let current = Local
                    .timestamp_millis_opt(after)
                    .single()
                    .ok_or("Invalid current timestamp")?;
                for days in 0..3 {
                    let date = current.date_naive() + Duration::days(days);
                    if let Some(candidate) = date
                        .and_hms_opt(base.hour(), base.minute(), base.second())
                        .and_then(|v| Local.from_local_datetime(&v).earliest())
                    {
                        if candidate.timestamp_millis() > after {
                            return Ok(candidate.timestamp_millis());
                        }
                    }
                }
                Err("Cannot resolve next daily workflow time".into())
            } else if first {
                Ok(after + 1)
            } else {
                Err("One-time workflow timer has already fired".into())
            }
        }
        "cron" => next_cron(&field(config, "cron_expression"), after),
        other => Err(format!("Unknown workflow schedule_type: {other}")),
    }
}
fn cron_set(raw: &str, min: u32, max: u32) -> Result<BTreeSet<u32>, String> {
    let mut output = BTreeSet::new();
    for part in raw.split(',') {
        let (base, step) = if let Some((base, step)) = part.split_once('/') {
            (
                base,
                step.parse::<u32>()
                    .map_err(|_| format!("Invalid cron step: {part}"))?,
            )
        } else {
            (part, 1)
        };
        if step == 0 {
            return Err("Cron step must be positive".into());
        }
        let (start, end) = if base == "*" {
            (min, max)
        } else if let Some((a, b)) = base.split_once('-') {
            (
                a.parse::<u32>()
                    .map_err(|_| format!("Invalid cron range: {base}"))?,
                b.parse::<u32>()
                    .map_err(|_| format!("Invalid cron range: {base}"))?,
            )
        } else {
            let value = base
                .parse::<u32>()
                .map_err(|_| format!("Invalid cron field: {base}"))?;
            (value, if part.contains('/') { max } else { value })
        };
        if start < min || end > max || start > end {
            return Err(format!("Cron field out of range: {raw}"));
        }
        for value in (start..=end).step_by(step as usize) {
            output.insert(value);
        }
    }
    Ok(output)
}
fn next_cron(expression: &str, after: i64) -> Result<i64, String> {
    let fields = expression.split_whitespace().collect::<Vec<_>>();
    if fields.len() != 5 {
        return Err("Workflow cron must have five fields: minute hour day month weekday".into());
    }
    let minutes = cron_set(fields[0], 0, 59)?;
    let hours = cron_set(fields[1], 0, 23)?;
    let days = cron_set(fields[2], 1, 31)?;
    let months = cron_set(fields[3], 1, 12)?;
    let weekdays = cron_set(fields[4], 0, 7)?;
    let current = Local
        .timestamp_millis_opt(after)
        .single()
        .ok_or("Invalid current timestamp")?;
    for offset in 0..(366 * 8) {
        let date = current.date_naive() + Duration::days(offset);
        if !months.contains(&date.month()) {
            continue;
        }
        let dom = days.contains(&date.day());
        let dow = date.weekday().num_days_from_sunday();
        let dow = weekdays.contains(&dow) || (dow == 0 && weekdays.contains(&7));
        let day_matches = if fields[2] != "*" && fields[4] != "*" {
            dom || dow
        } else {
            dom && dow
        };
        if !day_matches {
            continue;
        }
        for hour in &hours {
            for minute in &minutes {
                let Some(local) = date.and_hms_opt(*hour, *minute, 0) else {
                    continue;
                };
                let resolved = Local.from_local_datetime(&local);
                for candidate in [resolved.earliest(), resolved.latest()]
                    .into_iter()
                    .flatten()
                {
                    if candidate.timestamp_millis() > after {
                        return Ok(candidate.timestamp_millis());
                    }
                }
            }
        }
    }
    Err(format!(
        "Cron has no execution date in the next eight years: {expression}"
    ))
}

#[derive(Clone)]
struct NodeResult {
    status: &'static str,
    output: String,
}
fn reference(value: &Value) -> Option<String> {
    ["nodeId", "ref", "refNodeId"]
        .iter()
        .find_map(|key| value.get(*key).and_then(Value::as_str))
        .map(str::to_string)
}
fn resolve(value: &Value, results: &BTreeMap<String, NodeResult>) -> Result<String, String> {
    if let Some(id) = reference(value) {
        let result = results
            .get(&id)
            .ok_or_else(|| format!("Referenced workflow node has not completed: {id}"))?;
        if result.status == "FAILED" {
            return Err(format!(
                "Referenced workflow node failed: {id}: {}",
                result.output
            ));
        }
        return Ok(result.output.clone());
    }
    Ok(value.get("value").map(text).unwrap_or_else(|| text(value)))
}
fn references(node: &Value) -> Vec<String> {
    let mut refs = Vec::new();
    for key in ["left", "right", "source"] {
        if let Some(value) = reference(&node[key]) {
            refs.push(value);
        }
    }
    for value in node["actionConfig"]
        .as_object()
        .into_iter()
        .flat_map(|v| v.values())
    {
        if let Some(value) = reference(value) {
            refs.push(value);
        }
    }
    for value in node["others"].as_array().into_iter().flatten() {
        if let Some(value) = reference(value) {
            refs.push(value);
        }
    }
    refs
}
fn bool_like(raw: &str) -> Option<bool> {
    match raw.trim().to_lowercase().as_str() {
        "true" | "1" | "yes" | "y" | "on" => Some(true),
        "false" | "0" | "no" | "n" | "off" => Some(false),
        _ => None,
    }
}
fn edge_matches(
    edge: &Value,
    nodes: &BTreeMap<String, Value>,
    results: &BTreeMap<String, NodeResult>,
) -> bool {
    let source = field(edge, "sourceNodeId");
    let Some(result) = results.get(&source) else {
        return false;
    };
    if result.status == "SKIPPED" {
        return false;
    }
    let mut condition = field(edge, "condition").trim().to_string();
    if condition.is_empty()
        && nodes
            .get(&source)
            .is_some_and(|node| matches!(field(node, "type").as_str(), "condition" | "logic"))
    {
        condition = "true".into();
    }
    match condition.to_lowercase().as_str() {
        "on_error" | "error" | "failed" => result.status == "FAILED",
        "on_success" | "success" | "ok" | "" => result.status == "SUCCESS",
        "true" => result.status == "SUCCESS" && bool_like(&result.output).unwrap_or(false),
        "false" => result.status == "SUCCESS" && !bool_like(&result.output).unwrap_or(false),
        _ => {
            result.status == "SUCCESS"
                && regex::Regex::new(&condition)
                    .map(|r| r.is_match(&result.output))
                    .unwrap_or(false)
        }
    }
}
fn execute(
    runtime: &ToolPkgBridgeRuntime,
    workflow: &Value,
    trigger: Option<String>,
    extras: Value,
) -> Result<(bool, Value), String> {
    let nodes = array(&workflow["nodes"])?;
    let edges = array(&workflow["connections"])?;
    let by_id = nodes
        .iter()
        .map(|n| (field(n, "id"), n.clone()))
        .collect::<BTreeMap<_, _>>();
    let starts = if let Some(trigger) = trigger {
        if !by_id.contains_key(&trigger) {
            return Err(format!("Trigger node not found: {trigger}"));
        }
        vec![trigger]
    } else {
        let triggers = nodes
            .iter()
            .filter(|n| field(n, "type") == "trigger")
            .map(|n| field(n, "id"))
            .collect::<Vec<_>>();
        if triggers.is_empty() {
            nodes
                .iter()
                .filter(|n| !edges.iter().any(|e| e["targetNodeId"] == n["id"]))
                .map(|n| field(n, "id"))
                .collect()
        } else {
            triggers
        }
    };
    if starts.is_empty() {
        return Err("Workflow has no executable start node".into());
    }
    let mut dependencies = BTreeMap::<String, BTreeSet<String>>::new();
    for node in &nodes {
        dependencies.insert(field(node, "id"), references(node).into_iter().collect());
    }
    for edge in &edges {
        dependencies
            .entry(field(edge, "targetNodeId"))
            .or_default()
            .insert(field(edge, "sourceNodeId"));
    }
    let mut reachable = starts.iter().cloned().collect::<BTreeSet<_>>();
    loop {
        let previous = reachable.len();
        for (target, sources) in &dependencies {
            if sources.iter().any(|source| reachable.contains(source)) {
                reachable.insert(target.clone());
            }
        }
        if reachable.len() == previous {
            break;
        }
    }
    // Parameter references may require an upstream node outside the forward
    // branch. Include its dependencies without firing unrelated trigger nodes.
    loop {
        let previous = reachable.len();
        let ancestors = reachable
            .iter()
            .flat_map(|id| dependencies.get(id).into_iter().flatten())
            .filter(|id| by_id.contains_key(*id))
            .cloned()
            .collect::<Vec<_>>();
        reachable.extend(ancestors);
        if reachable.len() == previous {
            break;
        }
    }
    reachable.retain(|id| {
        by_id
            .get(id)
            .is_none_or(|node| field(node, "type") != "trigger" || starts.contains(id))
    });
    let mut results = BTreeMap::<String, NodeResult>::new();
    for start in &starts {
        if by_id
            .get(start)
            .is_some_and(|n| field(n, "type") == "trigger")
        {
            results.insert(
                start.clone(),
                NodeResult {
                    status: "SUCCESS",
                    output: extras.to_string(),
                },
            );
        }
    }
    while results.len() < reachable.len() {
        let ready =
            reachable
                .iter()
                .filter(|id| {
                    !results.contains_key(*id)
                        && dependencies.get(*id).into_iter().flatten().all(|source| {
                            !reachable.contains(source) || results.contains_key(source)
                        })
                })
                .cloned()
                .collect::<Vec<_>>();
        if ready.is_empty() {
            return Err("Workflow contains a dependency cycle or a missing node reference".into());
        }
        for id in ready {
            let node = by_id
                .get(&id)
                .ok_or_else(|| format!("Missing workflow node: {id}"))?;
            let incoming = edges
                .iter()
                .filter(|e| {
                    field(e, "targetNodeId") == id && reachable.contains(&field(e, "sourceNodeId"))
                })
                .collect::<Vec<_>>();
            let should_run = incoming.is_empty()
                || incoming
                    .iter()
                    .any(|edge| edge_matches(edge, &by_id, &results));
            if !should_run {
                results.insert(
                    id,
                    NodeResult {
                        status: "SKIPPED",
                        output: "Connection condition did not match".into(),
                    },
                );
                continue;
            }
            let result = run_node(runtime, node, &incoming, &results, &extras);
            results.insert(
                id,
                match result {
                    Ok(output) => NodeResult {
                        status: "SUCCESS",
                        output,
                    },
                    Err(output) => NodeResult {
                        status: "FAILED",
                        output,
                    },
                },
            );
        }
    }
    let failed = results.iter().any(|(id, result)| {
        result.status == "FAILED"
            && !edges.iter().any(|edge| {
                field(edge, "sourceNodeId") == *id
                    && matches!(
                        field(edge, "condition").to_lowercase().as_str(),
                        "on_error" | "error" | "failed"
                    )
                    && results
                        .get(&field(edge, "targetNodeId"))
                        .is_some_and(|r| r.status == "SUCCESS")
            })
    });
    let output = results
        .iter()
        .map(|(id, result)| {
            (
                id.clone(),
                json!({"status":result.status,"result":result.output}),
            )
        })
        .collect::<serde_json::Map<_, _>>();
    Ok((!failed, Value::Object(output)))
}
fn run_node(
    runtime: &ToolPkgBridgeRuntime,
    node: &Value,
    incoming: &[&Value],
    results: &BTreeMap<String, NodeResult>,
    extras: &Value,
) -> Result<String, String> {
    match field(node, "type").as_str() {
        "trigger" => Ok(extras.to_string()),
        "condition" => compare(
            &resolve(&node["left"], results)?,
            &resolve(&node["right"], results)?,
            &field(node, "operator"),
        )
        .map(|v| v.to_string()),
        "logic" => {
            let inputs = incoming
                .iter()
                .filter_map(|edge| results.get(&field(edge, "sourceNodeId")))
                .filter(|r| r.status == "SUCCESS")
                .filter_map(|r| bool_like(&r.output))
                .collect::<Vec<_>>();
            Ok(if field(node, "operator").to_uppercase() == "OR" {
                inputs.iter().any(|x| *x)
            } else {
                !inputs.is_empty() && inputs.iter().all(|x| *x)
            }
            .to_string())
        }
        "extract" => extract(node, incoming, results),
        "execute" => {
            let name = field(node, "actionType");
            if name.trim().is_empty() {
                return Err("Workflow execution node has no actionType".into());
            }
            let mut parameters = Vec::new();
            let mut payload = serde_json::Map::new();
            for (key, value) in node["actionConfig"]
                .as_object()
                .ok_or("Workflow actionConfig must be an object")?
            {
                let value = resolve(value, results)?;
                payload.insert(key.clone(), json!(value));
                parameters.push(ToolParameter {
                    name: key.clone(),
                    value,
                });
            }
            let action = match name.as_str() {
                "create_workflow" => Some("create"),
                "get_workflow" => Some("get"),
                "get_all_workflows" => Some("getAll"),
                "update_workflow" => Some("update"),
                "patch_workflow" => Some("patch"),
                "delete_workflow" => Some("delete"),
                "enable_workflow" => Some("enable"),
                "disable_workflow" => Some("disable"),
                "trigger_workflow" => None,
                _ => None,
            };
            if let Some(action) = action {
                return LegacyWorkflowService::command(runtime, action, Value::Object(payload))
                    .map(|v| text(&v));
            }
            if is_legacy_host_action(&name) {
                return execute_legacy_host_action(runtime, &name, Value::Object(payload));
            }
            let name = resolve_package_action(runtime, &name);
            let result = runtime
                .tool_handler()
                .executeTool(AITool { name, parameters });
            if result.success {
                if let ToolResultData::MessageSendResultData(data) = &result.result {
                    if let Some(reply) = data
                        .aiResponse
                        .as_value()
                        .filter(|reply| !reply.trim().is_empty())
                    {
                        return Ok(reply.clone());
                    }
                }
                Ok(result.result.toString())
            } else {
                Err(result.error.unwrap_or_else(|| result.result.toString()))
            }
        }
        other => Err(format!("Unsupported workflow node type: {other}")),
    }
}
fn compare(left: &str, right: &str, operator: &str) -> Result<bool, String> {
    let op = operator.to_uppercase();
    if op == "CONTAINS" {
        return Ok(left.contains(right));
    }
    if op == "NOT_CONTAINS" {
        return Ok(!left.contains(right));
    }
    if op == "IN" || op == "NOT_IN" {
        let items = serde_json::from_str::<Vec<Value>>(right)
            .map(|items| items.iter().map(text).collect::<Vec<_>>())
            .unwrap_or_else(|_| {
                right
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect()
            });
        let mut found = false;
        for item in items {
            if compare(left, &item, "EQ")? {
                found = true;
            }
        }
        return Ok(if op == "IN" { found } else { !found });
    }
    let order = match (left.trim().parse::<f64>(), right.trim().parse::<f64>()) {
        (Ok(a), Ok(b)) => a
            .partial_cmp(&b)
            .ok_or("Workflow condition cannot compare NaN")?,
        (Err(_), Err(_)) => left.cmp(right),
        _ => {
            return Err(format!(
                "Workflow condition mixes numeric and text values: {left}, {right}"
            ))
        }
    };
    Ok(match op.as_str() {
        "NE" => !order.is_eq(),
        "GT" => order.is_gt(),
        "GTE" => !order.is_lt(),
        "LT" => order.is_lt(),
        "LTE" => !order.is_gt(),
        _ => order.is_eq(),
    })
}
fn extract(
    node: &Value,
    incoming: &[&Value],
    results: &BTreeMap<String, NodeResult>,
) -> Result<String, String> {
    let mode = field(node, "mode").to_uppercase();
    let fallback = field(node, "defaultValue");
    if mode == "RANDOM_INT" || mode == "RANDOM_STRING" {
        if flag(&node["useFixed"], false) {
            let value = field(node, "fixedValue");
            if mode == "RANDOM_INT" {
                value
                    .parse::<i64>()
                    .map_err(|_| "Workflow fixed random integer is invalid")?;
            }
            return Ok(value);
        }
        if mode == "RANDOM_INT" {
            let a = number(&node["randomMin"], 0);
            let b = number(&node["randomMax"], 100);
            let (min, max) = (a.min(b), a.max(b));
            let span = (max as i128 - min as i128 + 1) as u128;
            let random = Uuid::new_v4().as_u128() % span;
            return Ok((min as i128 + random as i128).to_string());
        }
        let charset = if field(node, "randomStringCharset").is_empty() {
            "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789".into()
        } else {
            field(node, "randomStringCharset")
        };
        let chars = charset.chars().collect::<Vec<_>>();
        let length = number(&node["randomStringLength"], 8).max(0) as usize;
        return Ok((0..length)
            .map(|_| chars[(Uuid::new_v4().as_u128() % (chars.len() as u128)) as usize])
            .collect());
    }
    let mut source = resolve(&node["source"], results)?;
    if source.trim().is_empty() && reference(&node["source"]).is_none() {
        if let Some(result) = incoming
            .first()
            .and_then(|edge| results.get(&field(edge, "sourceNodeId")))
            .filter(|r| r.status == "SUCCESS")
        {
            source = result.output.clone();
        }
    }
    match mode.as_str() {
        "REGEX" => {
            let pattern = field(node, "expression");
            if pattern.is_empty() {
                return Ok(fallback);
            }
            Ok(regex::Regex::new(&pattern)
                .ok()
                .and_then(|r| {
                    r.captures(&source).and_then(|c| {
                        c.get(number(&node["group"], 0).max(0) as usize)
                            .map(|m| m.as_str().to_string())
                    })
                })
                .unwrap_or(fallback))
        }
        "JSON" => {
            let Ok(mut value) = serde_json::from_str::<Value>(&source) else {
                return Ok(fallback);
            };
            let path = field(node, "expression");
            let path = path.trim().trim_start_matches('$').trim_start_matches('.');
            if path.starts_with('/') {
                return Ok(value
                    .pointer(path)
                    .filter(|v| !v.is_null())
                    .map(text)
                    .unwrap_or(fallback));
            }
            let parts = path.replace('[', ".").replace(']', "");
            for key in parts.split('.').filter(|k| !k.is_empty()) {
                let key = key.trim_matches('\'').trim_matches('"');
                value = if let Some(object) = value.as_object() {
                    object.get(key).cloned().unwrap_or(Value::Null)
                } else if let Ok(index) = key.parse::<usize>() {
                    value.get(index).cloned().unwrap_or(Value::Null)
                } else {
                    Value::Null
                };
            }
            Ok(if value.is_null() {
                fallback
            } else {
                text(&value)
            })
        }
        "SUB" => {
            let chars = source.encode_utf16().collect::<Vec<_>>();
            let raw_start = number(&node["startIndex"], 0);
            if raw_start < 0 || chars.is_empty() {
                return Ok(fallback);
            }
            let start = raw_start as usize;
            if start > chars.len() {
                return Ok(fallback);
            }
            let length = number(&node["length"], -1);
            let end = if length < 0 {
                chars.len()
            } else {
                start.saturating_add(length as usize).min(chars.len())
            };
            Ok(String::from_utf16_lossy(&chars[start..end]))
        }
        "CONCAT" => {
            for other in node["others"].as_array().into_iter().flatten() {
                source.push_str(&resolve(other, results)?);
            }
            Ok(source)
        }
        other => Err(format!("Unknown workflow extract mode: {other}")),
    }
}

fn is_legacy_host_action(name: &str) -> bool {
    matches!(
        name,
        "trigger_workflow"
            | "execute_shell"
            | "execute_intent"
            | "send_broadcast"
            | "get_page_info"
            | "capture_screenshot"
            | "tap"
            | "long_press"
            | "click_element"
            | "set_input_text"
            | "press_key"
            | "swipe"
            | "run_ui_subagent"
            | "ffmpeg_execute"
            | "ffmpeg_info"
            | "ffmpeg_convert"
            | "call_chat_model"
            | "get_chat_messages_range"
            | "list_sandbox_packages"
            | "set_sandbox_package_enabled"
            | "execute_sandbox_script_direct"
            | "restart_mcp_with_logs"
            | "get_speech_services_config"
            | "set_speech_services_config"
            | "test_tts_playback"
            | "list_model_configs"
            | "create_model_config"
            | "update_model_config"
            | "delete_model_config"
            | "list_function_model_configs"
            | "get_function_model_config"
            | "set_function_model_config"
            | "test_model_config_connection"
    )
}
#[cfg(feature = "javascript")]
fn execute_legacy_host_action(
    runtime: &ToolPkgBridgeRuntime,
    action: &str,
    payload: Value,
) -> Result<String, String> {
    let engine = operit_js_bridge::javascript::JsEngine::JsEngine::new(std::sync::Arc::new(
        runtime.tool_handler(),
    ));
    let script = format!(
        "{}\nexports.__operitLegacyWorkflowAction = __operitLegacyWorkflowAction;",
        include_str!("LegacyWorkflowTools.js")
    );
    let params = BTreeMap::from([
        ("action".into(), json!(action)),
        ("payload".into(), payload),
    ]);
    let result = engine.execute_script_function(
        &script,
        "__operitLegacyWorkflowAction",
        &params,
        &BTreeMap::new(),
        None,
        false,
        180,
        None,
    );
    engine.destroy();
    let output = result
        .map_err(|error| error.to_string())?
        .unwrap_or_default();
    let value = serde_json::from_str::<Value>(&output).unwrap_or(json!(output));
    if value.get("success") == Some(&Value::Bool(false))
        || value.get("executionSuccess") == Some(&Value::Bool(false))
    {
        return Err(value
            .get("error")
            .or_else(|| value.get("executionError"))
            .or_else(|| value.get("message"))
            .map(text)
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| value.to_string()));
    }
    Ok(text(&value))
}
#[cfg(not(feature = "javascript"))]
fn execute_legacy_host_action(
    _runtime: &ToolPkgBridgeRuntime,
    action: &str,
    _payload: Value,
) -> Result<String, String> {
    Err(format!(
        "Workflow action requires the JavaScript host: {action}"
    ))
}

fn resolve_package_action(runtime: &ToolPkgBridgeRuntime, name: &str) -> String {
    let Some((package, function)) = name.split_once(':') else {
        return name.into();
    };
    let handler = runtime.tool_handler();
    let manager = handler.getOrCreatePackageManager();
    let mut manager = manager.lock().unwrap_or_else(|error| error.into_inner());
    let candidates = if package.starts_with("legacy_") || package.starts_with("legacy.") {
        vec![package.to_string()]
    } else {
        vec![
            format!("legacy_{package}"),
            format!("legacy.{package}"),
            package.to_string(),
        ]
    };
    for candidate in candidates {
        if manager.getPackageTools(&candidate).is_some() {
            if !manager.isPackageEnabled(&candidate) {
                manager.enablePackage(&candidate);
            }
            manager.usePackage(&candidate);
            return format!("{candidate}:{function}");
        }
    }
    name.into()
}
