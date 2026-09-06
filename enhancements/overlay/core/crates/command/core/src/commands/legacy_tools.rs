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
    if let Some(resolved)=resolved {model.parameters=resolved.parameters;model.contextOverride=Some(resolved.context);model.capabilitiesOverride=Some(resolved.capabilities);}
    let old_model_id=model.id.clone();
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
    if !model.id.is_empty(){if provider.models.is_empty(){provider.models.push(model.clone());}else{provider.models[0]=model.clone();}}
    let saved=manager.updateProviderProfile(provider).map_err(|e|e.to_string())?;
    let mut affected=Vec::new();
    if !old_model_id.is_empty()&&old_model_id!=model.id{for name in FUNCTIONS{let f=function(name)?;let manager=FunctionalConfigManager::default();let binding=manager.getModelBindingForFunction(f.clone()).map_err(|e|e.to_string())?;if binding.providerId==id&&binding.modelId==old_model_id{manager.setModelForFunction(f,id.clone(),model.id.clone()).map_err(|e|e.to_string())?;affected.push(name.to_string());}}}
    Ok(json!({"created":created,"updated":!created,"config":provider_result(&saved,0)?,"changedFields":map.keys().collect::<Vec<_>>(),"affectedFunctions":affected}))
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
        let finished=tokio::select! {
            _=stream.collect(&mut |chunk|text.push_str(&chunk))=>true,
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
        "model-delete"=>{let id=string(&request,"id")?;let affected=FUNCTIONS.iter().filter_map(|f|function_mapping(f).ok().filter(|v|v["configId"]==id).map(|_|f.to_string())).collect::<Vec<_>>();models.deleteProvider(id).map_err(|e|e.to_string())?;json!({"deleted":true,"configId":id,"affectedFunctions":affected,"fallbackConfigId":function_mapping("CHAT")?["configId"]})},
        "function-list"=>json!({"defaultConfigId":function_mapping("CHAT")?["configId"],"mappings":FUNCTIONS.iter().map(|f|function_mapping(f)).collect::<Result<Vec<_>,_>>()?}),
        "function-get"=>{let mut v=function_mapping(string(&request,"id")?)?;v["defaultConfigId"]=function_mapping("CHAT")?["configId"].clone();v},
        "function-set"=>{let name=string(&request,"functionType")?;let id=string(&request,"configId")?;let provider=models.getProviderProfile(id).map_err(|e|e.to_string())?;let index=request["modelIndex"].as_u64().unwrap_or(0) as usize;let model=provider.models.get(index).ok_or("modelIndex out of range")?;FunctionalConfigManager::default().setModelForFunction(function(name)?,id.to_string(),model.id.clone()).map_err(|e|e.to_string())?;let mut result=function_mapping(name)?;result["requestedModelIndex"]=json!(index);result},
        "model-test"=>{let provider=models.getProviderProfile(string(&request,"configId")?).map_err(|e|e.to_string())?;let index=request["modelIndex"].as_u64().unwrap_or(0) as usize;let model=provider.models.get(index).ok_or("modelIndex out of range")?;run_async(async {
            let report=application.test_model_connection(provider.id.clone(),model.id.clone()).await?;
            let passed=report.items.iter().filter(|i|i.success).count();
            Ok(json!({"configId":provider.id,"configName":provider.name,"providerType":provider.providerTypeId,"requestedModelIndex":index,"actualModelIndex":index,"testedModelName":model.id,"success":report.success,"totalTests":report.items.len(),"passedTests":passed,"failedTests":report.items.len()-passed,"tests":report.items}))
        })?},
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
        "speech-test"=>speech_test(application,&request)?,
        "script-run"=>script_run(application,&request)?,
        "mcp-restart"=>mcp_restart(application,&request)?,
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
    let stt=SttConfigManager::getInstance().getCurrentSttConfig()?;
    Ok(json!({"ttsServiceType":if tts.providerType=="SYSTEM_TTS"{"SIMPLE_TTS"}else{&tts.providerType},
        "ttsHttpConfig":{"urlTemplate":tts.endpoint,"apiKeySet":!tts.apiKey.is_empty(),"apiKeyPreview":if tts.apiKey.is_empty(){""}else{"configured"},
          "headers":tts.headers.iter().map(|h|(h.name.clone(),json!(h.value))).collect::<serde_json::Map<String,Value>>(),
          "httpMethod":tts.httpMethod,"requestBody":tts.requestBody,"contentType":tts.contentType,
          "localeTag":if tts.providerType=="SYSTEM_TTS"{tts.model.clone()}else{String::new()},"voiceId":tts.voice,"modelName":tts.model,"responsePipeline":tts.responsePipeline},
        "ttsCleanerRegexs":[],"ttsSpeechRate":tts.speed,"ttsPitch":1.0,
        "sttServiceType":if stt.providerType=="OPENAI_COMPATIBLE"{"OPENAI_STT"}else{&stt.providerType},
        "sttHttpConfig":{"endpointUrl":stt.endpoint,"apiKeySet":!stt.apiKey.is_empty(),"apiKeyPreview":if stt.apiKey.is_empty(){""}else{"configured"},"modelName":stt.model}}))
}
fn speech_set(request:&Value)->Result<Value,String>{
    use operit_runtime::data::preferences::{TtsConfigManager::TtsConfigManager,SttConfigManager::SttConfigManager};
    let tts_manager=TtsConfigManager::getInstance();let stt_manager=SttConfigManager::getInstance();
    let mut tts=encode(tts_manager.getCurrentTtsConfig()?)?;let mut stt=encode(stt_manager.getCurrentSttConfig()?)?;
    let updates=request.as_object().ok_or("Speech updates must be object")?;
    for (key,value) in updates{
        match key.as_str(){
            "tts_service_type"=>tts["providerType"]=json!(match value.as_str().ok_or("tts_service_type must be string")?{"SIMPLE_TTS"=>"SYSTEM_TTS",other=>other}),
            "stt_service_type"=>stt["providerType"]=json!(match value.as_str().ok_or("stt_service_type must be string")?{"OPENAI_STT"=>"OPENAI_COMPATIBLE",other=>other}),
            "tts_url_template"=>tts["endpoint"]=value.clone(),"tts_api_key"=>tts["apiKey"]=value.clone(),"tts_model_name"|"tts_locale"=>tts["model"]=value.clone(),
            "tts_voice_id"=>tts["voice"]=value.clone(),"tts_speech_rate"=>tts["speed"]=value.clone(),
            "tts_http_method"=>tts["httpMethod"]=value.clone(),"tts_request_body"=>tts["requestBody"]=value.clone(),"tts_content_type"=>tts["contentType"]=value.clone(),
            "tts_headers"=>tts["headers"]=json_value(value)?,"tts_response_pipeline"=>tts["responsePipeline"]=json_value(value)?,
            "stt_endpoint_url"=>stt["endpoint"]=value.clone(),"stt_api_key"=>stt["apiKey"]=value.clone(),"stt_model_name"=>stt["model"]=value.clone(),
            _=>return Err(format!("Speech setting needs a host implementation: {key}"))
        }
    }
    let tts:operit_model::TtsConfig::TtsConfig=serde_json::from_value(tts).map_err(|e|e.to_string())?;
    let stt:operit_model::SttConfig::SttConfig=serde_json::from_value(stt).map_err(|e|e.to_string())?;
    if updates.keys().any(|k|k.starts_with("tts_")){tts_manager.updateTtsConfig(tts.clone())?;}
    if updates.keys().any(|k|k.starts_with("stt_")){stt_manager.updateSttConfig(stt.clone())?;}
    Ok(json!({"updated":!updates.is_empty(),"changedFields":updates.keys().collect::<Vec<_>>(),"ttsServiceType":tts.providerType,"sttServiceType":stt.providerType,"ttsApiKeySet":!tts.apiKey.is_empty(),"sttApiKeySet":!stt.apiKey.is_empty()}))
}
fn speech_test(application:&OperitApplication,request:&Value)->Result<Value,String>{
    use operit_runtime::data::preferences::TtsConfigManager::TtsConfigManager;
    use operit_runtime::services::{TtsPlaybackService::TtsPlaybackService,TtsSynthesisService::TtsSynthesisService};
    let config=TtsConfigManager::getInstance().getCurrentTtsConfig()?;
    let text=string(request,"text")?;
    let interrupt=request["options"]["interrupt"].as_bool().unwrap_or(true);
    let playback=TtsPlaybackService::getInstance(&application.hostManager)?;
    let started=if config.providerType=="SYSTEM_TTS"{
        playback.speakWithConfig(&config.id,text,interrupt)?.active
    }else{
        let audio=TtsSynthesisService::getInstance(&application.hostManager).synthesizeWithConfig(&config.id,text)?;
        let mut started=false;for path in audio.audioPaths{started|=playback.playAudio(&path)?.started;}started
    };
    Ok(json!({"ttsServiceType":config.providerType,"providerClass":"Operit2 configured TTS provider","initialized":true,"playbackTriggered":started,"interrupt":interrupt,"textLength":text.chars().count(),"speechRate":config.speed,"pitch":1.0}))
}
fn script_run(application:&OperitApplication,request:&Value)->Result<Value,String>{
    use std::collections::BTreeMap;
    let source=match request.get("source_code").and_then(Value::as_str){Some(code) if !code.is_empty()=>code.to_string(),_=>std::fs::read_to_string(string(request,"source_path")?).map_err(|e|e.to_string())?};
    let params=request.get("params_json").map(json_value).transpose()?.unwrap_or(json!({}));
    let params:BTreeMap<String,Value>=serde_json::from_value(params).map_err(|e|e.to_string())?;
    let mut env=BTreeMap::new();
    if let Some(path)=request["env_file_path"].as_str().filter(|p|!p.is_empty()){
        for line in std::fs::read_to_string(path).map_err(|e|e.to_string())?.lines(){let line=line.trim();if line.is_empty()||line.starts_with('#'){continue;}if let Some((key,value))=line.split_once('='){env.insert(key.trim().to_string(),value.trim().trim_matches('"').trim_matches('\'').to_string());}}
    }
    let engine=application.toolHandler.runtimeDependencies().js_execution_provider().create_execution_engine(std::sync::Arc::new(application.toolHandler.clone()));
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
fn command_json(application:&OperitApplication,args:&[String])->Result<Value,String>{
    let mut output=CoreCommandOutput::new();output.setJsonMode(true);
    super::mcp::run_mcp_command(application,args,&mut output)?;output.finalizeJson()?;
    serde_json::from_str(&output.stdout).map_err(|e|e.to_string())
}
fn mcp_restart(application:&OperitApplication,request:&Value)->Result<Value,String>{
    let started=now();let timeout=request["timeoutMs"].as_i64().or_else(||request["timeoutMs"].as_str().and_then(|s|s.parse().ok())).unwrap_or(60000);
    let servers=command_json(application,&["list".into()])?;
    let mut rows=Vec::new();let mut timed_out=false;
    for item in servers.as_array().ok_or("MCP list must be array")?{
        if item["enabled"]==false{continue;}
        if now()-started>=timeout{timed_out=true;break;}
        let id=string(item,"id")?;
        let result=command_json(application,&["kill".into(),id.into()]).and_then(|_|command_json(application,&["start".into(),id.into()]));
        match result{Ok(result)=>rows.push(json!({"id":id,"status":"success","message":"Restarted","log":result.to_string()})),Err(error)=>rows.push(json!({"id":id,"status":"error","message":error,"log":error}))}
    }
    let successes=rows.iter().filter(|r|r["status"]=="success").count();
    Ok(json!({"timeoutMs":timeout,"elapsedMs":now()-started,"timedOut":timed_out,"progress":if timed_out{0}else{100},"message":"MCP restart finished","pluginsTotal":servers.as_array().unwrap().len(),"pluginsStarted":rows.len(),"successCount":successes,"failedCount":rows.len()-successes,"plugins":rows,"extraLogs":{"package":application.packageLogText()?}}))
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
