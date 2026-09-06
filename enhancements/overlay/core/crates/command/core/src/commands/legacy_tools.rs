//! Operit1 package contracts implemented by the current managers, enhanced channel only.
use crate::output::CoreCommandOutput;
use operit_model::ModelConfigData::{ApiProviderType, ModelProfile, ProviderProfile};
use operit_model::ModelParameter::{ModelParameter, ParameterValueType};
use operit_model::CharacterCard::CharacterCard;
use operit_runtime::core::application::OperitApplication::OperitApplication;
use operit_runtime::data::preferences::ModelConfigManager::ModelConfigManager;
use operit_runtime::data::preferences::FunctionalConfigManager::FunctionalConfigManager;
use operit_runtime::data::preferences::CharacterCardManager::CharacterCardManager;
use operit_model::FunctionType::FunctionType;
use serde_json::{json, Value};
use operit_util::stream::Stream::Stream;
use operit_providers::runtime_support::ProviderRuntimeContext;
use operit_tools::tools::AIToolHandler::AIToolHandler;

fn string<'a>(value: &'a Value, key: &str) -> Result<&'a str, String> {
    value.get(key).and_then(Value::as_str).ok_or_else(||format!("{key} must be a string"))
}
fn encode<T: serde::Serialize>(value: T) -> Result<Value,String> {
    serde_json::to_value(value).map_err(|e|e.to_string())
}
fn now() -> i64 { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as i64 }
fn json_value(value: &Value) -> Result<Value,String> {
    match value { Value::String(s) => serde_json::from_str(s).map_err(|e|e.to_string()), _=>Ok(value.clone()) }
}
const RETAINED_KEY:&str="OPERIT2_RETAINED_LEGACY_SETTINGS";
fn retained_settings(section:&str)->Result<Value,String>{
    use operit_runtime::data::preferences::EnvPreferences::EnvPreferences;
    let all:Value=match EnvPreferences::getInstance().getEnv(RETAINED_KEY).map_err(|e|e.to_string())? {Some(raw)=>serde_json::from_str(&raw).map_err(|e|e.to_string())?,None=>json!({})};
    Ok(all.get(section).cloned().unwrap_or(json!({})))
}
fn retain_settings(section:&str,updates:&Value)->Result<(),String>{
    use operit_runtime::data::preferences::EnvPreferences::EnvPreferences;
    let store=EnvPreferences::getInstance();
    let mut all:Value=match store.getEnv(RETAINED_KEY).map_err(|e|e.to_string())? {Some(raw)=>serde_json::from_str(&raw).map_err(|e|e.to_string())?,None=>json!({})};
    if !all[section].is_object(){all[section]=json!({});}
    for (key,value) in updates.as_object().ok_or("Retained legacy settings must be an object")? {all[section][key]=value.clone();}
    store.setEnv(RETAINED_KEY,&all.to_string()).map_err(|e|e.to_string())
}
fn retained_notice(fields:&[String])->Vec<String>{
    if fields.is_empty(){Vec::new()}else{vec![format!("以下旧版设置已原样保留，但 Operit2 当前没有对应执行引擎，未启用或替换现有设置：{}。请在增强版设置页选择可用服务；不要将保留成功视为功能已生效。",fields.join(", "))]}
}
fn active_card(manager: &CharacterCardManager) -> Result<Value,String> {
    Ok(json!(manager.observeActiveCharacterCardId().first().map_err(|e|e.to_string())?))
}
fn card_result(card: CharacterCard) -> Result<Value,String> {
    let mut value = encode(card)?;
    value["chatModelBindingMode"]=json!(if value["chatModelBindingMode"]=="FIXED_MODEL"{"FIXED_CONFIG"}else{"FOLLOW_GLOBAL"});
    value["chatModelConfigId"]=Value::Null;
    if let Some(model_id)=value["chatModelId"].as_str().map(str::to_string){
        for provider in ModelConfigManager::default().getProviderProfiles().map_err(|e|e.to_string())? {
            if let Some(index)=provider.models.iter().position(|m|m.id==model_id){value["chatModelConfigId"]=json!(provider.id);value["chatModelIndex"]=json!(index);break;}
        }
    }
    if value.get("chatModelIndex").is_none(){value["chatModelIndex"] = json!(0);}
    value["memoryProfileBindingMode"] = json!(if value["memoryBindingMode"] == "SHARED" {"FIXED_PROFILE"}else{"FOLLOW_GLOBAL"});
    value["memoryProfileId"] = value["sharedMemoryId"].clone();
    Ok(value)
}
fn provider_result(provider: &ProviderProfile, index: usize) -> Result<Value,String> {
    let model = provider.models.get(index);
    let mut result = json!({"id":provider.id,"name":provider.name,"apiProviderType":provider.providerTypeId,
        "apiEndpoint":provider.endpoint,"apiKeySet":!provider.apiKey.is_empty(),"apiKeyPreview":if provider.apiKey.is_empty(){""}else{"configured"},
        "modelName":model.map(|m|m.id.clone()).unwrap_or_default(),"modelList":provider.models.iter().map(|m|m.id.clone()).collect::<Vec<_>>(),
        "customHeaders":provider.customHeaders,"hasCustomHeaders":provider.customHeaders!="{}",
        "useMultipleApiKeys":provider.useMultipleApiKeys,"apiKeyPoolCount":provider.apiKeyPool.len(),
        "requestLimitPerMinute":provider.requestLimitPerMinute,"maxConcurrentRequests":provider.maxConcurrentRequests,
        "customParameters":"{}","hasCustomParameters":false});
    if let Some(model)=model {
        let manager=ModelConfigManager::default();
        let resolved=manager.getResolvedModelConfig(&provider.id,&model.id).map_err(|e|e.to_string())?;
        let data=encode(&resolved)?;
        for (key, val) in encode(&model.localRuntime)?.as_object().expect("object") {result[key]=val.clone();}
        for (key, val) in encode(&model.summary)?.as_object().expect("object") {result[key]=val.clone();}
        result["enableGoogleSearch"]=json!(resolved.builtinTools.iter().any(|tool|tool.enabled&&tool.requestFormat==operit_model::ModelConfigData::BuiltinToolRequestFormat::GeminiGoogleSearch));
        result["contextLength"]=data["context"]["maxContextLength"].clone();
        result["maxContextLength"]=data["context"]["maxContextLength"].clone();
        result["enableMaxContextMode"]=data["context"]["enableMaxContextMode"].clone();
        for (old,new) in [("enableDirectImageProcessing","directImage"),("enableDirectAudioProcessing","directAudio"),("enableDirectVideoProcessing","directVideo"),("enableToolCall","toolCall")] {result[old]=data["capabilities"][new].clone();}
        for name in ["max_tokens","temperature","top_p","top_k","presence_penalty","frequency_penalty","repetition_penalty"] {
            let camel=camel(name);
            let param=resolved.parameters.iter().find(|p|p.apiName==name);
            result[&camel]=param.map(|p|p.currentValue.clone()).unwrap_or(json!(0));
            result[&(camel+"Enabled")]=json!(param.map(|p|p.isEnabled).unwrap_or(false));
        }
        let custom:serde_json::Map<String,Value>=resolved.parameters.iter().filter(|p|p.isCustom&&p.isEnabled).map(|p|(p.apiName.clone(),p.currentValue.clone())).collect();
        result["hasCustomParameters"]=json!(!custom.is_empty());result["customParameters"]=json!(serde_json::to_string(&custom).map_err(|e|e.to_string())?);
    }
    let retained=retained_settings(&format!("model:{}",provider.id))?;
    result["retainedLegacySettings"]=retained.clone();
    let fields=retained.as_object().map(|v|v.keys().cloned().collect::<Vec<_>>()).unwrap_or_default();
    result["compatibilityWarnings"]=json!(retained_notice(&fields));
    Ok(result)
}
fn camel(name:&str)->String {
    let mut out=String::new();let mut upper=false;
    for c in name.chars(){if c=='_'{upper=true;}else if upper{out.extend(c.to_uppercase());upper=false;}else{out.push(c);}}out
}
fn function(value:&str)->Result<FunctionType,String>{serde_json::from_value(json!(value)).map_err(|e|e.to_string())}
fn function_mapping(name:&str)->Result<Value,String>{
    let b=FunctionalConfigManager::default().getModelBindingForFunction(function(name)?).map_err(|e|e.to_string())?;
    let p=ModelConfigManager::default().getProviderProfile(&b.providerId).map_err(|e|e.to_string())?;
    let index=p.models.iter().position(|m|m.id==b.modelId).ok_or_else(||"Bound model does not exist".to_string())?;
    Ok(json!({"functionType":name,"configId":p.id,"configName":p.name,"modelIndex":index,"actualModelIndex":index,"selectedModel":b.modelId,"config":provider_result(&p,index)?}))
}
const FUNCTIONS:&[&str]=&["CHAT","SUMMARY","TITLE_GENERATION","MEMORY","UI_CONTROLLER","TRANSLATION","GREP","ROLE_RESPONSE_PLANNER","IMAGE_RECOGNITION","AUDIO_RECOGNITION","VIDEO_RECOGNITION"];
fn write_model(id:Option<&str>, updates:&Value)->Result<Value,String>{
    let map=updates.as_object().ok_or_else(||"Model updates must be an object".to_string())?;
    let manager=ModelConfigManager::default();
    let created=id.is_none();
    let id=match id {Some(id)=>id.to_string(),None=>manager.createProvider(
        updates["name"].as_str().unwrap_or("Legacy model").to_string(),
        updates["api_provider_type"].as_str().unwrap_or("OPENAI_GENERIC").to_string(),
        updates["api_endpoint"].as_str().unwrap_or("https://api.openai.com/v1/chat/completions").to_string()
    ).map_err(|e|e.to_string())?};
    let mut provider=manager.getProviderProfile(&id).map_err(|e|e.to_string())?;
    let mut model=provider.models.first().cloned().unwrap_or_else(||ModelProfile::new(String::new()));
    let resolved=if model.id.is_empty(){None}else{Some(manager.getResolvedModelConfig(&id,&model.id).map_err(|e|e.to_string())?)};
    if let Some(resolved)=resolved {model.parameters=resolved.parameters;model.contextOverride=Some(resolved.context);model.capabilitiesOverride=Some(resolved.capabilities);model.builtinToolsOverride=Some(resolved.builtinTools);}
    let previous_model_ids=provider.models.iter().map(|m|m.id.clone()).collect::<Vec<_>>();
    let mut provider_json=encode(&provider)?;
    let mut model_json=encode(&model)?;
    for (key,value) in map {
        match key.as_str(){
            "name"=>provider_json["name"]=value.clone(),
            "api_endpoint"=>provider_json["endpoint"]=value.clone(),
            "api_key"=>provider_json["apiKey"]=value.clone(),
            "api_provider_type"=>{let kind=ApiProviderType::fromProviderTypeId(value.as_str().ok_or("api_provider_type must be a string")?).ok_or("Unknown provider type")?;provider_json["providerType"]=encode(&kind)?;provider_json["providerTypeId"]=json!(kind.name());},
            "custom_headers"=>provider_json["customHeaders"]=json!(if value.is_string(){value.as_str().unwrap().to_string()}else{value.to_string()}),
            "model_name"=>model_json["id"]=value.clone(),
            "request_limit_per_minute"|"max_concurrent_requests"=>provider_json[camel(key)]=value.clone(),
            "max_context_length"|"context_length"|"enable_max_context_mode"=>{
                if model_json["contextOverride"].is_null(){model_json["contextOverride"]=encode(operit_model::ModelConfigData::ModelContextSpec::default())?;}
                let target=if key=="context_length"{"maxContextLength".to_string()}else{camel(key)};model_json["contextOverride"][target]=value.clone();
            },
            "enable_summary"|"summary_token_threshold"|"enable_summary_by_message_count"|"summary_message_count_threshold"=>model_json["summary"][camel(key)]=value.clone(),
            "enable_direct_image_processing"|"enable_direct_audio_processing"|"enable_direct_video_processing"|"enable_tool_call"=>{
                if model_json["capabilitiesOverride"].is_null(){model_json["capabilitiesOverride"]=encode(operit_model::ModelConfigData::ModelCapabilities::default())?;}
                let target=match key.as_str(){"enable_direct_image_processing"=>"directImage","enable_direct_audio_processing"=>"directAudio","enable_direct_video_processing"=>"directVideo",_=>"toolCall"};model_json["capabilitiesOverride"][target]=value.clone();
            },
            k if k.starts_with("mnn_")||k.starts_with("llama_")=>model_json["localRuntime"][camel(k)]=value.clone(),
            "enable_google_search"=>{
                let enabled=value.as_bool().ok_or("enable_google_search must be boolean")?;
                if model_json["builtinToolsOverride"].is_null(){model_json["builtinToolsOverride"]=json!([]);}
                let tools=model_json["builtinToolsOverride"].as_array_mut().ok_or("Missing model built-in tool defaults")?;
                if let Some(tool)=tools.iter_mut().find(|tool|tool["requestFormat"]=="GeminiGoogleSearch"){tool["enabled"]=json!(enabled);}
                else{tools.push(json!({"toolType":"WebSearch","displayName":"Google Search","enabled":enabled,"requestFormat":"GeminiGoogleSearch","exclusivity":"CanMixWithExternalTools","config":{}}));}
            },
            "enable_claude_1h_prompt_cache"=>{value.as_bool().ok_or("enable_claude_1h_prompt_cache must be boolean")?;},
            "custom_parameters"=>{},
            k if ["max_tokens","temperature","top_p","top_k","presence_penalty","frequency_penalty","repetition_penalty"].iter().any(|p| k==*p||k==format!("{p}_enabled"))=>{},
            _=>return Err(format!("Operit2 has no equivalent model field: {key}")),
        }
    }
    model=serde_json::from_value(model_json).map_err(|e|e.to_string())?;
    for name in ["max_tokens","temperature","top_p","top_k","presence_penalty","frequency_penalty","repetition_penalty"] {
        let flag=format!("{name}_enabled");
        if !map.contains_key(name)&&!map.contains_key(&flag){continue;}
        let found=model.parameters.iter().position(|p|p.apiName==name);
        let index=match found {Some(i)=>i,None=>{let p=ModelParameter::new(name.to_string(),name.to_string(),name.to_string(),json!(0),json!(0),false,if name=="max_tokens"||name=="top_k"{ParameterValueType::INT}else{ParameterValueType::FLOAT});model.parameters.push(p);model.parameters.len()-1}};
        if let Some(v)=map.get(name){model.parameters[index].currentValue=v.clone();}
        if let Some(v)=map.get(&flag){model.parameters[index].isEnabled=v.as_bool().ok_or("enabled must be boolean")?;}
    }
    if let Some(custom)=map.get("custom_parameters"){
        let custom=json_value(custom)?;let custom=custom.as_object().ok_or("custom_parameters must be object")?;
        model.parameters.retain(|p|!p.isCustom);
        for (name,value) in custom{let mut p=ModelParameter::new(name.clone(),name.clone(),name.clone(),value.clone(),value.clone(),true,if value.is_boolean(){ParameterValueType::BOOLEAN}else if value.is_i64(){ParameterValueType::INT}else if value.is_number(){ParameterValueType::FLOAT}else if value.is_string(){ParameterValueType::STRING}else{ParameterValueType::OBJECT});p.isCustom=true;model.parameters.push(p);}
    }
    provider=serde_json::from_value(provider_json).map_err(|e|e.to_string())?;
    if let Some(names)=updates["model_name"].as_str(){
        let names=names.split(',').map(str::trim).filter(|n|!n.is_empty()).collect::<Vec<_>>();
        let mut selected=Vec::new();
        for name in names {
            let mut profile=model.clone();profile.id=name.to_string();selected.push(profile);
        }
        provider.models=selected;
        if let Some(first)=provider.models.first(){model=first.clone();}
    }else if !model.id.is_empty(){if provider.models.is_empty(){provider.models.push(model.clone());}else{provider.models[0]=model.clone();}}
    if provider.models.is_empty(){
        for name in FUNCTIONS{if FunctionalConfigManager::default().getModelBindingForFunction(function(name)?).map_err(|e|e.to_string())?.providerId==id{return Err("A bound provider must retain at least one model".into());}}
    }
    let saved=manager.updateProviderProfile(provider).map_err(|e|e.to_string())?;
    let mut affected=Vec::new();
    for name in FUNCTIONS{
        let role=function(name)?;let functions=FunctionalConfigManager::default();let binding=functions.getModelBindingForFunction(role.clone()).map_err(|e|e.to_string())?;
        if binding.providerId==id && !saved.models.iter().any(|m|m.id==binding.modelId){
            let index=previous_model_ids.iter().position(|m|m==&binding.modelId).unwrap_or(0);
            let replacement=saved.models.get(index.min(saved.models.len().saturating_sub(1))).ok_or("A bound provider must retain at least one model")?;
            functions.setModelForFunction(role,id.clone(),replacement.id.clone()).map_err(|e|e.to_string())?;affected.push(name.to_string());
        }
    }
    let retained:serde_json::Map<String,Value>=map.iter().filter(|(key,_)|key.as_str()=="enable_claude_1h_prompt_cache").map(|(key,value)|(key.clone(),value.clone())).collect();
    retain_settings(&format!("model:{id}"),&Value::Object(retained.clone()))?;
    let retained_fields=retained.keys().cloned().collect::<Vec<_>>();
    Ok(json!({"created":created,"updated":!created&&map.keys().any(|key|!retained.contains_key(key)),"config":provider_result(&saved,0)?,"changedFields":map.keys().filter(|key|!retained.contains_key(*key)).collect::<Vec<_>>(),"retainedFields":retained_fields,"compatibilityWarnings":retained_notice(&retained_fields),"affectedFunctions":affected}))
}
fn write_card(id:Option<&str>, updates:&Value, application:&mut OperitApplication)->Result<Value,String>{
    let manager=CharacterCardManager::getInstance();
    let created=id.is_none();
    let id=match id{Some(id)=>id.to_string(),None=>{
        let mut output=CoreCommandOutput::new();output.setJsonMode(true);
        super::people::run_character_command(application,&["create".into(),updates["name"].as_str().unwrap_or("Legacy character").into()],&mut output)?;
        output.finalizeJson()?;let value:Value=serde_json::from_str(&output.stdout).map_err(|e|e.to_string())?;string(&value,"id")?.to_string()
    }};
    let mut card=encode(manager.getCharacterCard(&id).map_err(|e|e.to_string())?)?;
    let map=updates.as_object().ok_or("Character updates must be object")?;
    for (key,value) in map{
        match key.as_str(){
            "name"|"description"|"character_setting"|"opening_statement"|"other_content_chat"|"other_content_voice"|"advanced_custom_prompt"|"marks"=>card[camel(key)]=value.clone(),
            "attached_tag_ids"=>card["attachedTagIds"]=json_value(value)?,
            "chat_model_binding_mode"=>card["chatModelBindingMode"]=json!(if value=="FIXED_CONFIG"{"FIXED_MODEL"}else{"FOLLOW_GLOBAL"}),
            "chat_model_config_id"=>{
                let provider_id=value.as_str().ok_or("chat_model_config_id must be string")?;
                if provider_id.is_empty(){card["chatModelId"]=Value::Null;}else{
                    let provider=ModelConfigManager::default().getProviderProfile(provider_id).map_err(|e|e.to_string())?;
                    let index=updates["chat_model_index"].as_u64().unwrap_or(0) as usize;
                    let model=provider.models.get(index).ok_or("chat_model_index out of range")?;
                    card["chatModelId"]=json!(model.id);
                }
            },
            "chat_model_index"=>{},
            "memory_profile_binding_mode"=>card["memoryBindingMode"]=json!(if value=="FIXED_PROFILE"{"SHARED"}else{"CHARACTER"}),
            "memory_profile_id"=>card["sharedMemoryId"]=if value==""{Value::Null}else{value.clone()},
            "tool_access_enabled"=>card["toolAccessConfig"]["enabled"]=value.clone(),
            "allowed_builtin_tools"|"allowed_packages"|"allowed_skills"|"allowed_mcp_servers"=>card["toolAccessConfig"][camel(key)]=json_value(value)?,
            _=>return Err(format!("Unknown character field: {key}"))
        }
    }
    let card:CharacterCard=serde_json::from_value(card).map_err(|e|e.to_string())?;
    manager.updateCharacterCard(card.clone()).map_err(|e|e.to_string())?;
    Ok(json!({"created":created,"updated":!created,"card":card_result(card)?,"activeCharacterCardId":active_card(&manager)?,"changedFields":map.keys().collect::<Vec<_>>()}))
}
fn parse_turns(value:&Value)->Result<Vec<operit_model::PromptTurn::PromptTurn>,String>{
    let values=value.as_array().ok_or("turns must be array")?;
    values.iter().map(|v|{let mut v=v.clone();if v.get("metadata").is_none(){v["metadata"]=json!({});}serde_json::from_value(v).map_err(|e|e.to_string())}).collect()
}
fn run_async<T>(future:impl std::future::Future<Output=Result<T,String>>)->Result<T,String>{
    // Core commands execute on the existing blocking tool worker, not the Tokio event loop.
    tokio::runtime::Builder::new_current_thread().enable_all().build().map_err(|e|e.to_string())?.block_on(future)
}
async fn chat_call(provider_context:ProviderRuntimeContext,request:&Value)->Result<Value,String>{
    use operit_providers::chat::enhance::MultiServiceManager::MultiServiceManager;
    use operit_providers::chat::llmprovider::AIService::SendMessageRequest;
    use operit_runtime::data::preferences::ApiPreferences::ApiPreferences;
    let mut manager=MultiServiceManager::from_runtime_context(provider_context).map_err(|e|e.to_string())?;
    let (config,parameters,handle)=manager.getServiceBundleForFunction(function(string(request,"functionType")?)?).map_err(|e|e.to_string())?;
    let turns=parse_turns(&request["turns"])?;
    {
        let mut service=handle.lock().await;
        let mut stream=service.send_message(SendMessageRequest{chat_history:turns,model_parameters:parameters,
            enable_thinking:request["enableThinking"].as_bool().unwrap_or(false),stream:false,available_tools:Vec::new(),
            preserve_think_in_history:false,enable_retry:true,on_non_fatal_error:None,on_tool_invocation:None}).await.map_err(|e|e.to_string())?;
        let mut raw=String::new();stream.collect(&mut|chunk|raw.push_str(&chunk)).await;
        if request["recordTokenUsage"].as_bool().unwrap_or(true){ApiPreferences::getInstance().updateTokensForProviderModel(&format!("{}:{}",config.providerId,config.modelId),service.input_token_count(),service.output_token_count(),service.cached_input_token_count()).map_err(|e|e.to_string())?;}
        let clean=operit_util::ChatMarkupRegex::ChatMarkupRegex::remove_gemini_thought_signature_meta(&raw);
        let calls=operit_util::ChatMarkupRegex::ChatMarkupRegex::tool_call_matches(&clean);
        let mut turns=Vec::new();let mut text=String::new();let mut cursor=0;
        for call in &calls {
            if call.start>cursor {let part=&clean[cursor..call.start];text.push_str(part);turns.push(json!({"kind":"ASSISTANT","content":part,"metadata":{}}));}
            turns.push(json!({"kind":"TOOL_CALL","content":&clean[call.start..call.end],"toolName":call.name,"metadata":{}}));cursor=call.end;
        }
        if cursor<clean.len(){let part=&clean[cursor..];text.push_str(part);turns.push(json!({"kind":"ASSISTANT","content":part,"metadata":{}}));}
        Ok(json!({"text":text,"turns":turns,"finishReason":if calls.is_empty(){"stop"}else{"tool_call"},"metadata":{},"receivedAt":now(),"inputTokens":service.input_token_count(),"outputTokens":service.output_token_count(),"cachedInputTokens":service.cached_input_token_count()}))
    }
}
async fn wait_cancelled(cancelled:&std::sync::atomic::AtomicBool){
    while !cancelled.load(std::sync::atomic::Ordering::Acquire){tokio::time::sleep(std::time::Duration::from_millis(100)).await;}
}
async fn chat_plan_call(handler:AIToolHandler,provider_context:ProviderRuntimeContext,request:&Value)->Result<Value,String>{
    use operit_providers::chat::EnhancedAIService::{EnhancedAIService,SendMessageOptions};
    let mut service=EnhancedAIService::new(handler,provider_context);
    let mut options=SendMessageOptions::new();
    options.message=string(request,"message")?.to_string();
    options.chatHistory=match request.get("chatHistory"){Some(value)=>parse_turns(value)?,None=>Vec::new()};
    options.maxTokens=request["maxTokens"].as_i64().unwrap_or(0) as i32;
    options.tokenUsageThreshold=request["tokenUsageThreshold"].as_f64().unwrap_or(0.0);
    options.workspacePath=request["workspacePath"].as_str().map(str::to_string);
    options.customSystemPromptTemplate=request["customSystemPromptTemplate"].as_str().map(str::to_string);
    options.proxySenderName=request["proxySenderName"].as_str().map(str::to_string);
    options.isSubTask=request["isSubTask"].as_bool().unwrap_or(false);
    if options.isSubTask && options.customSystemPromptTemplate.is_none(){options.customSystemPromptTemplate=Some(operit_providers::chat::config::SystemPromptConfig::SUBTASK_AGENT_PROMPT_TEMPLATE.to_string());}
    options.enableMemoryAutoUpdate=request["enableMemoryAutoUpdate"].as_bool().unwrap_or(false);
    options.enableThinking=request["enableThinking"].as_bool().unwrap_or(false);
    options.stream=false;options.disableWarning=true;
    let request_id=string(request,"requestId")?.to_string();
    let cancelled=operit_plugin_sdk::legacy_planning_cancellation::register(&request_id,request["parentExecutionId"].as_str().unwrap_or(""))?;
    let result=async {
        let response=tokio::select! {
            response=service.sendMessage(options)=>Some(response),
            _=wait_cancelled(&cancelled)=>None
        };
        let mut stream=match response {Some(result)=>result.map_err(|e|e.to_string())?,None=>{service.cancelConversation().await;return Err("Subtask cancelled".into());}};
        let mut text=String::new();
        let mut collect_chunk=|chunk: String|text.push_str(&chunk);
        let finished=tokio::select! {
            _=stream.collect(&mut collect_chunk)=>true,
            _=wait_cancelled(&cancelled)=>false
        };
        if !finished{service.cancelConversation().await;return Err("Subtask cancelled".into());}
        Ok(json!({"text":text,"inputTokens":service.getCurrentInputTokenCount(),"outputTokens":service.getCurrentOutputTokenCount(),"cachedInputTokens":service.getCurrentCachedInputTokenCount()}))
    }.await;
    operit_plugin_sdk::legacy_planning_cancellation::remove(&request_id);
    result
}

pub fn run(application:&mut OperitApplication,args:&[String],output:&mut CoreCommandOutput)->Result<(),String>{
    let action=args.first().ok_or("legacy-tools action required")?;
    let request:Value=serde_json::from_str(args.get(1).map(String::as_str).unwrap_or("{}")).map_err(|e|e.to_string())?;
    let models=ModelConfigManager::default();
    let cards=CharacterCardManager::getInstance();
    let value=match action.as_str(){
        "model-list"=>{
            let providers=models.getProviderProfiles().map_err(|e|e.to_string())?;
            let bindings=FUNCTIONS.iter().map(|f|function_mapping(f)).collect::<Result<Vec<_>,_>>()?;
            json!({"totalConfigCount":providers.len(),"defaultConfigId":function_mapping("CHAT")?["configId"],"configs":providers.iter().map(|p|provider_result(p,0)).collect::<Result<Vec<_>,_>>()?,"functionMappings":bindings})
        },
        "model-create"=>write_model(None,&request)?,
        "model-update"=>write_model(Some(string(&request,"id")?),&request["updates"])?,
        "model-delete"=>{
            let id=string(&request,"id")?;if id=="LOCAL_MODEL"{return Err("The built-in local model provider cannot be deleted".into());}let functions=FunctionalConfigManager::default();
            let mut affected=Vec::new();
            for name in FUNCTIONS {let role=function(name)?;let binding=functions.getModelBindingForFunction(role.clone()).map_err(|e|e.to_string())?;if binding.providerId==id{affected.push((name.to_string(),role));}}
            let providers=models.getProviderProfiles().map_err(|e|e.to_string())?;
            let replacement=providers.iter().find(|p|p.id!=id&&!p.models.is_empty());
            if !affected.is_empty()&&replacement.is_none(){return Err("Add another model configuration before deleting the last bound provider".into());}
            // Rebind valid functions before deleting the provider so no dangling model ids are saved.
            if let Some(replacement)=replacement{for (_,role) in &affected{functions.setModelForFunction(role.clone(),replacement.id.clone(),replacement.models[0].id.clone()).map_err(|e|e.to_string())?;}}
            models.deleteProvider(id).map_err(|e|e.to_string())?;
            json!({"deleted":true,"configId":id,"affectedFunctions":affected.iter().map(|(name,_)|name).collect::<Vec<_>>(),"fallbackConfigId":replacement.map(|p|p.id.clone())})
        },
        "function-list"=>json!({"defaultConfigId":function_mapping("CHAT")?["configId"],"mappings":FUNCTIONS.iter().map(|f|function_mapping(f)).collect::<Result<Vec<_>,_>>()?}),
        "function-get"=>{let mut v=function_mapping(string(&request,"id")?)?;v["defaultConfigId"]=function_mapping("CHAT")?["configId"].clone();v},
        "function-set"=>{let name=string(&request,"functionType")?;let id=string(&request,"configId")?;let provider=models.getProviderProfile(id).map_err(|e|e.to_string())?;let index=request["modelIndex"].as_u64().unwrap_or(0) as usize;let model=provider.models.get(index).ok_or("modelIndex out of range")?;FunctionalConfigManager::default().setModelForFunction(function(name)?,id.to_string(),model.id.clone()).map_err(|e|e.to_string())?;let mut result=function_mapping(name)?;result["requestedModelIndex"]=json!(index);result},
        "model-test"=>start_blocking_job(application,"model-test",request.clone())?,
        "character-list"=>json!({"cards":cards.getAllCharacterCards().map_err(|e|e.to_string())?.into_iter().map(card_result).collect::<Result<Vec<_>,_>>()?,"totalCount":cards.getAllCharacterCards().map_err(|e|e.to_string())?.len(),"activeCharacterCardId":active_card(&cards)?}),
        "character-get"=>json!({"card":card_result(cards.getCharacterCard(string(&request,"id")?).map_err(|e|e.to_string())?)?,"activeCharacterCardId":active_card(&cards)?}),
        "character-create"=>write_card(None,&request,application)?,
        "character-update"=>write_card(Some(string(&request,"id")?),&request["updates"],application)?,
        "character-delete"=>{let id=string(&request,"id")?;cards.deleteCharacterCard(id).map_err(|e|e.to_string())?;json!({"deleted":true,"characterCardId":id,"activeCharacterCardId":active_card(&cards)?})},
        "character-active"=>{cards.setActiveCharacterCard(string(&request,"id")?).map_err(|e|e.to_string())?;json!({"activeCharacterCardId":active_card(&cards)?})},
        "character-clear"=>{cards.clearActiveCharacterCard().map_err(|e|e.to_string())?;json!({"activeCharacterCardId":active_card(&cards)?})},
        "character-import"=>{let id=cards.createCharacterCardFromTavernJson(string(&request,"tavernJson")?)?;json!({"imported":true,"card":card_result(cards.getCharacterCard(&id).map_err(|e|e.to_string())?)?,"activeCharacterCardId":active_card(&cards)?})},
        "character-export"=>{let id=string(&request,"id")?;json!({"characterCardId":id,"tavernJson":cards.exportCharacterCardToTavernJson(id)?})},
        "chat-range"=>{
            let manager=operit_store::repository::ChatHistoryManager::ChatHistoryManager::default().map_err(|e|e.to_string())?;
            let id=string(&request,"chatId")?;let start=request["start"].as_u64().ok_or("start must be nonnegative integer")? as usize;let end=request["end"].as_u64().ok_or("end must be nonnegative integer")? as usize;
            if end<start{return Err("end must be >= start".into());}
            let order=request["order"].as_str().unwrap_or("asc");
            let mut messages=manager.loadChatMessages(id).map_err(|e|e.to_string())?;messages.retain(|m|m.sender!="summary");if order=="desc"{messages.reverse();}
            let selected=messages.into_iter().skip(start).take(end-start+1).map(|m|json!({"content":m.displayText(),"sender":m.sender,"timestamp":m.timestamp,"roleName":m.roleName,"provider":m.provider,"modelName":m.modelName})).collect::<Vec<_>>();
            json!({"chatId":id,"order":order,"limit":end-start+1,"messages":selected})
        },
        "speech-get"=>speech_get()?,
        "speech-set"=>speech_set(&request)?,
        "speech-test"=>start_blocking_job(application,"speech-test",request.clone())?,
        "script-run"=>start_blocking_job(application,"script-run",request.clone())?,
        "mcp-restart"=>start_blocking_job(application,"mcp-restart",request.clone())?,
        "package-errors"=>{
            let log=application.packageLogText()?;let mut errors=serde_json::Map::new();
            for line in log.lines(){if let Some((_,detail))=line.split_once("package load error ["){if let Some((name,message))=detail.split_once("]: "){errors.insert(name.to_string(),json!(message));}}}
            Value::Object(errors)
        },
        "chat-call"=>start_model_job(application,"chat-call",request.clone())?,
        "chat-plan-call"=>start_model_job(application,"chat-plan-call",request.clone())?,
        "model-job"=>poll_model_job(&request)?,
        "chat-plan-cancel"=>{
            let id=string(&request,"requestId")?;json!({"requestId":id,"cancelled":operit_plugin_sdk::legacy_planning_cancellation::cancel(id)})
        },
        _=>return Err(format!("Unknown legacy-tools action: {action}"))
    };
    output.setJsonStdout(value);Ok(())
}
fn speech_get()->Result<Value,String>{
    use operit_runtime::data::preferences::{TtsConfigManager::TtsConfigManager,SttConfigManager::SttConfigManager};
    let tts=TtsConfigManager::getInstance().getCurrentTtsConfig()?;
    let extras=operit_runtime::services::LegacySpeechSettings::read()?;
    let stt=SttConfigManager::getInstance().getCurrentSttConfig()?;
    let retained=retained_settings("speech")?;let retained_fields=retained.as_object().map(|v|v.keys().cloned().collect::<Vec<_>>()).unwrap_or_default();
    Ok(json!({"ttsServiceType":if tts.providerType=="SYSTEM_TTS"{"SIMPLE_TTS"}else{&tts.providerType},
        "ttsHttpConfig":{"urlTemplate":tts.endpoint,"apiKeySet":!tts.apiKey.is_empty(),"apiKeyPreview":if tts.apiKey.is_empty(){""}else{"configured"},
          "headers":tts.headers.iter().map(|h|(h.name.clone(),json!(h.value))).collect::<serde_json::Map<String,Value>>(),
          "httpMethod":tts.httpMethod,"requestBody":tts.requestBody,"contentType":tts.contentType,
          "localeTag":if tts.providerType=="SYSTEM_TTS"{tts.model.clone()}else{String::new()},"voiceId":tts.voice,"modelName":tts.model,"responsePipeline":tts.responsePipeline},
        "retainedFields":retained_fields,"compatibilityWarnings":retained_notice(&retained_fields),"ttsCleanerRegexs":extras["tts_cleaner_regexs"],"ttsSpeechRate":tts.speed,"ttsPitch":extras["tts_pitch"],
        "sttServiceType":if stt.providerType=="OPENAI_COMPATIBLE"{"OPENAI_STT"}else{&stt.providerType},
        "sttHttpConfig":{"endpointUrl":stt.endpoint,"apiKeySet":!stt.apiKey.is_empty(),"apiKeyPreview":if stt.apiKey.is_empty(){""}else{"configured"},"modelName":stt.model}}))
}
fn speech_set(request:&Value)->Result<Value,String>{
    use operit_runtime::data::preferences::{TtsConfigManager::TtsConfigManager,SttConfigManager::SttConfigManager};
    let tts_manager=TtsConfigManager::getInstance();let stt_manager=SttConfigManager::getInstance();
    let mut tts=encode(tts_manager.getCurrentTtsConfig()?)?;let mut stt=encode(stt_manager.getCurrentSttConfig()?)?;
    let original=request.as_object().ok_or("Speech updates must be object")?;
    let tts_unsupported=request["tts_service_type"].as_str().map(|kind|!matches!(kind,"SIMPLE_TTS"|"OPENAI_TTS")&&operit_model::TtsCatalog::TtsCatalog::provider(kind).is_err()).unwrap_or(false);
    let stt_unsupported=request["stt_service_type"].as_str().map(|kind|kind!="OPENAI_STT"&&operit_model::SttCatalog::SttCatalog::provider(kind).is_err()).unwrap_or(false);
    let retained:serde_json::Map<String,Value>=original.iter().filter(|(key,_)|key.starts_with("tts_vits_")||(tts_unsupported&&key.starts_with("tts_"))||(stt_unsupported&&key.starts_with("stt_"))).map(|(key,value)|(key.clone(),value.clone())).collect();
    let applied=Value::Object(original.iter().filter(|(key,_)|!retained.contains_key(*key)).map(|(key,value)|(key.clone(),value.clone())).collect());
    let request=&applied;let updates=request.as_object().unwrap();
    if let Some(kind)=request["tts_service_type"].as_str(){
        let kind=match kind {"SIMPLE_TTS"=>"SYSTEM_TTS","OPENAI_TTS"=>"OPENAI_COMPATIBLE",other=>other};
        if tts["providerType"]!=kind {
            let catalog=operit_model::TtsCatalog::TtsCatalog::provider(kind)?;
            tts["providerType"]=json!(kind);tts["endpoint"]=json!(catalog.defaultEndpoint);tts["model"]=json!(catalog.defaultModel);
            tts["responseFormat"]=json!(catalog.defaultResponseFormat);tts["httpMethod"]=json!(catalog.defaultHttpMethod);
            tts["contentType"]=json!(catalog.defaultContentType);tts["requestBody"]=json!(catalog.defaultRequestBody);
            tts["headers"]=encode(catalog.defaultHeaders)?;tts["responsePipeline"]=encode(catalog.defaultResponsePipeline)?;
        }
    }

    for (key,value) in updates{
        match key.as_str(){
            "tts_service_type"=>tts["providerType"]=json!(match value.as_str().ok_or("tts_service_type must be string")?{"SIMPLE_TTS"=>"SYSTEM_TTS","OPENAI_TTS"=>"OPENAI_COMPATIBLE",other=>other}),
            "stt_service_type"=>stt["providerType"]=json!(match value.as_str().ok_or("stt_service_type must be string")?{"OPENAI_STT"=>"OPENAI_COMPATIBLE",other=>other}),
            "tts_url_template"=>tts["endpoint"]=value.clone(),"tts_api_key"=>tts["apiKey"]=value.clone(),"tts_model_name"|"tts_locale"=>tts["model"]=value.clone(),
            "tts_voice_id"=>tts["voice"]=value.clone(),"tts_speech_rate"=>tts["speed"]=value.clone(),
            "tts_http_method"=>tts["httpMethod"]=value.clone(),"tts_request_body"=>tts["requestBody"]=value.clone(),"tts_content_type"=>tts["contentType"]=value.clone(),
            "tts_pitch"|"tts_cleaner_regexs"=>{},
            "tts_headers"=>{let value=json_value(value)?;tts["headers"]=match value.as_object(){Some(map)=>json!(map.iter().map(|(name,value)|json!({"name":name,"value":value})).collect::<Vec<_>>()),None=>value};},
            "tts_response_pipeline"=>{let mut pipeline=json_value(value)?;for step in pipeline.as_array_mut().ok_or("tts_response_pipeline must be an array")?{if let Some(kind)=step.get("type").cloned(){step["stepType"]=kind;}if step.get("headers").is_none(){step["headers"]=json!([]);}else if let Some(headers)=step["headers"].as_object(){step["headers"]=json!(headers.iter().map(|(name,value)|json!({"name":name,"value":value})).collect::<Vec<_>>());}}tts["responsePipeline"]=pipeline;},
            "stt_endpoint_url"=>stt["endpoint"]=value.clone(),"stt_api_key"=>stt["apiKey"]=value.clone(),"stt_model_name"=>stt["model"]=value.clone(),
            _=>return Err(format!("Speech setting needs a host implementation: {key}"))
        }
    }
    let tts:operit_model::TtsConfig::TtsConfig=serde_json::from_value(tts).map_err(|e|e.to_string())?;
    operit_runtime::services::LegacySpeechSettings::update(request)?;
    let stt:operit_model::SttConfig::SttConfig=serde_json::from_value(stt).map_err(|e|e.to_string())?;
    if updates.keys().any(|k|k.starts_with("tts_")){tts_manager.updateTtsConfig(tts.clone())?;}
    if updates.keys().any(|k|k.starts_with("stt_")){stt_manager.updateSttConfig(stt.clone())?;}
    retain_settings("speech",&Value::Object(retained.clone()))?;
    let retained_fields=retained.keys().cloned().collect::<Vec<_>>();
    Ok(json!({"retainedFields":retained_fields,"compatibilityWarnings":retained_notice(&retained_fields),"updated":!updates.is_empty(),"changedFields":updates.keys().collect::<Vec<_>>(),"ttsServiceType":tts.providerType,"sttServiceType":stt.providerType,"ttsApiKeySet":!tts.apiKey.is_empty(),"sttApiKeySet":!stt.apiKey.is_empty()}))
}
fn speech_test(host:&operit_host_api::HostManager::HostManager,request:&Value)->Result<Value,String>{
    use operit_runtime::data::preferences::TtsConfigManager::TtsConfigManager;
    use operit_runtime::services::{TtsPlaybackService::TtsPlaybackService,TtsSynthesisService::TtsSynthesisService};
    let mut config=TtsConfigManager::getInstance().getCurrentTtsConfig()?;
    if let Some(speed)=request["options"]["speech_rate"].as_f64(){config.speed=speed;}
    let pitch=request["options"]["pitch"].as_f64().unwrap_or(operit_runtime::services::LegacySpeechSettings::pitch()?);
    let text=string(request,"text")?;
    let cleaned=operit_runtime::services::LegacySpeechSettings::clean(text)?;
    let interrupt=request["options"]["interrupt"].as_bool().unwrap_or(true);
    let playback=TtsPlaybackService::getInstance(host)?;
    let started=if config.providerType=="SYSTEM_TTS"{
        host.ttsPlaybackHost.as_ref().ok_or("System TTS playback host is unavailable")?.speakText(operit_host_api::TtsPlaybackRequest {text:cleaned.clone(),voice:config.voice.clone(),locale:config.model.clone(),speed:config.speed,pitch,interrupt}).map_err(|e|e.to_string())?.active
    }else{
        let audio=TtsSynthesisService::getInstance(host).synthesizeWithResolvedConfig(&config.id,&config,&cleaned)?;
        let mut started=false;for path in audio.audioPaths{started|=playback.playAudio(&path)?.started;}started
    };
    Ok(json!({"ttsServiceType":config.providerType,"providerClass":"Operit2 configured TTS provider","initialized":true,"playbackTriggered":started,"interrupt":interrupt,"textLength":text.chars().count(),"speechRate":config.speed,"pitch":pitch}))
}
fn script_run(handler:AIToolHandler,request:&Value)->Result<Value,String>{
    use std::collections::BTreeMap;
    let source=match request.get("source_code").and_then(Value::as_str){Some(code) if !code.is_empty()=>code.to_string(),_=>std::fs::read_to_string(string(request,"source_path")?).map_err(|e|e.to_string())?};
    let params=request.get("params_json").map(json_value).transpose()?.unwrap_or(json!({}));
    let params:BTreeMap<String,Value>=serde_json::from_value(params).map_err(|e|e.to_string())?;
    let mut env=BTreeMap::new();
    if let Some(path)=request["env_file_path"].as_str().filter(|p|!p.is_empty()){
        for line in std::fs::read_to_string(path).map_err(|e|e.to_string())?.lines(){let line=line.trim();if line.is_empty()||line.starts_with('#'){continue;}if let Some((key,value))=line.split_once('='){env.insert(key.trim().to_string(),value.trim().trim_matches('"').trim_matches('\'').to_string());}}
    }
    let engine=handler.runtimeDependencies().js_execution_provider().create_execution_engine(std::sync::Arc::new(handler.clone()));
    // The legacy direct runner executes a script body with `params` and awaits complete().
    let source=format!("exports.__legacy_direct = async function(params) {{\n{source}\n}};");
    let started=now();let wait=request["wait_ms"].as_u64().or_else(||request["wait_ms"].as_str().and_then(|s|s.parse().ok())).unwrap_or(60000);
    let result=engine.execute_script_function_with_timeout_millis(&source,"__legacy_direct",&params,&env,None,false,wait);
    engine.destroy();
    match result{
        Ok(result)=>Ok(json!({"success":true,"result":result,"durationMs":now()-started,"sourcePath":request["source_path"],"scriptLabel":request["script_label"]})),
        Err(error)=>Err(error.message)
    }
}
fn mcp_restart(host:operit_host_api::HostManager::HostManager,mut handler:AIToolHandler,request:&Value)->Result<Value,String>{
    use operit_tools::tools::mcp_runtime::{MCPLocalServer::MCPLocalServer,plugins::{MCPBridge::MCPBridge,MCPStarter::MCPStarter}};
    use operit_tools::tools::mcp::MCPManager::MCPManager;
    let started=now();let timeout=request["timeoutMs"].as_i64().or_else(||request["timeoutMs"].as_str().and_then(|s|s.parse().ok())).unwrap_or(60000);
    let server=MCPLocalServer::getInstance(&host);let servers=server.getAllMCPServers();
    let mut rows=Vec::new();let mut timed_out=false;
    for (id,_) in &servers{
        if !server.isServerEnabled(id){continue;}
        let remaining=timeout-(now()-started);if remaining<=0{timed_out=true;break;}
        let bridge=MCPBridge::getInstance(&host).unregisterMcpService(id);
        if bridge["success"]!=true{rows.push(json!({"id":id,"status":"error","message":bridge["error"],"log":bridge.to_string()}));continue;}
        MCPManager::getInstance(host.clone()).unregisterServer(id);
        handler.unregisterMcpServerTools(id);handler.unregisterMcpServerPackage(id);
        let starter=MCPStarter::new(host.clone(),handler.runtimeSupport());let mut logs=Vec::new();
        let success=starter.startPluginWithTimeout(id,remaining as u64,|status|logs.push(format!("{status:?}")));
        server.updateServerStatus(id.clone(),None,None,if success{Some(now())}else{None},None)?;
        rows.push(json!({"id":id,"status":if success{"success"}else{"error"},"message":if success{"Restarted"}else{"Startup failed"},"log":logs.join("\n")}));
    }
    let successes=rows.iter().filter(|r|r["status"]=="success").count();
    Ok(json!({"timeoutMs":timeout,"elapsedMs":now()-started,"timedOut":timed_out,"progress":if timed_out{0}else{100},"message":"MCP restart finished","pluginsTotal":servers.len(),"pluginsStarted":rows.len(),"successCount":successes,"failedCount":rows.len()-successes,"plugins":rows,"extraLogs":{}}))
}
fn start_blocking_job(application:&OperitApplication,action:&str,request:Value)->Result<Value,String>{
    static NEXT:std::sync::atomic::AtomicU64=std::sync::atomic::AtomicU64::new(1);
    let id=format!("legacy-host-{}-{}",now(),NEXT.fetch_add(1,std::sync::atomic::Ordering::Relaxed));
    let state=json!({"executionId":id,"status":"RUNNING"});model_jobs().lock().map_err(|e|e.to_string())?.insert(id.clone(),state.clone());
    let host=application.hostManager.clone();let handler=application.toolHandler.clone();let context=application.providerRuntimeContext.clone();let action=action.to_string();let job_id=id.clone();
    let scheduler=operit_host_api::HostManager::defaultHostRuntimeTaskSchedulerHost();
    if let Err(error)=scheduler.scheduleHostRuntimeTask("legacy-host-operation",Box::new(move || {
        let result=match action.as_str(){
            "script-run"=>script_run(handler,&request),"speech-test"=>speech_test(&host,&request),"mcp-restart"=>mcp_restart(host,handler,&request),
            "model-test"=>run_async(async {
                let provider=ModelConfigManager::default().getProviderProfile(string(&request,"configId")?).map_err(|e|e.to_string())?;let index=request["modelIndex"].as_u64().unwrap_or(0) as usize;
                let model=provider.models.get(index).ok_or("modelIndex out of range")?;
                let report=ModelConfigManager::default().testModelConnection(&provider.id,&model.id,context).await.map_err(|e|e.to_string())?;
                let passed=report.items.iter().filter(|i|i.success).count();
                Ok(json!({"configId":provider.id,"configName":provider.name,"providerType":provider.providerTypeId,"requestedModelIndex":index,"actualModelIndex":index,"testedModelName":model.id,"success":report.success,"totalTests":report.items.len(),"passedTests":passed,"failedTests":report.items.len()-passed,"tests":report.items}))
            }),_=>Err(format!("Unknown host job: {action}"))
        };
        let state=match result{Ok(result)=>json!({"executionId":job_id,"status":"SUCCESS","result":result}),Err(error)=>json!({"executionId":job_id,"status":"FAILED","error":error})};
        if let Ok(mut jobs)=model_jobs().lock(){jobs.insert(job_id,state);}
    })){model_jobs().lock().map_err(|e|e.to_string())?.remove(&id);return Err(error.to_string());}
    Ok(state)
}

fn model_jobs()->&'static std::sync::Mutex<std::collections::HashMap<String,Value>> {
    static JOBS:std::sync::OnceLock<std::sync::Mutex<std::collections::HashMap<String,Value>>>=std::sync::OnceLock::new();
    JOBS.get_or_init(||std::sync::Mutex::new(std::collections::HashMap::new()))
}
fn start_model_job(application:&OperitApplication,action:&str,mut request:Value)->Result<Value,String>{
    static NEXT:std::sync::atomic::AtomicU64=std::sync::atomic::AtomicU64::new(1);
    let id=format!("legacy-model-{}-{}",now(),NEXT.fetch_add(1,std::sync::atomic::Ordering::Relaxed));
    if request["requestId"].as_str().unwrap_or("").is_empty(){request["requestId"]=json!(id);}
    let state=json!({"executionId":id,"status":"RUNNING"});
    model_jobs().lock().map_err(|e|e.to_string())?.insert(id.clone(),state.clone());
    let context=application.providerRuntimeContext.clone();let handler=application.toolHandler.clone();let action=action.to_string();
    let job_id=id.clone();
    let scheduler=operit_host_api::HostManager::defaultHostRuntimeTaskSchedulerHost();
    if let Err(error)=scheduler.scheduleHostRuntimeAsyncTask("legacy-model-request",Box::new(move || Box::pin(async move {
        let result=if action=="chat-call"{chat_call(context,&request).await}else{chat_plan_call(handler,context,&request).await};
        let status=match result{Ok(result)=>json!({"executionId":job_id,"status":"SUCCESS","result":result}),Err(error)=>json!({"executionId":job_id,"status":"FAILED","error":error})};
        if let Ok(mut jobs)=model_jobs().lock(){jobs.insert(job_id,status);}
    }))){model_jobs().lock().map_err(|e|e.to_string())?.remove(&id);return Err(error.to_string());}
    Ok(state)
}
fn poll_model_job(request:&Value)->Result<Value,String>{
    let id=string(request,"executionId")?;let mut jobs=model_jobs().lock().map_err(|e|e.to_string())?;
    let value=jobs.get(id).cloned().ok_or_else(||format!("Unknown model execution: {id}"))?;
    if value["status"]!="RUNNING"{jobs.remove(id);}
    Ok(value)
}
