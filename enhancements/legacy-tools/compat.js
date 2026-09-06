// Operit1 contracts over Operit2 executors. This factory never modifies native Tools.
function __operitCreateLegacyTools(base) {
  const api = Object.create(base);
  for (const name of ['Chat', 'Files', 'Net', 'SoftwareSettings', 'System', 'UI', 'FFmpeg', 'Workflow']) {
    api[name] = Object.create(base[name] || null);
  }
  const command = async (action, value) => JSON.parse(await base.SoftwareSettings.exec(
    ['--json', 'legacy-tools', action, JSON.stringify(value === undefined ? {} : value)]));
  const cli = async args => JSON.parse(await base.SoftwareSettings.exec(['--json'].concat(args)));
  const textResult = value => value !== null && typeof value === 'object'
    ? Object.assign(value, {toString() {return JSON.stringify(this);}}) : value;
  const modelRequest = async (action,options) => {
    const task=await command(action,options);
    while(true) {
      const state=await command('model-job',{executionId:task.executionId});
      if(state.status==='SUCCESS') return textResult(state.result);
      if(state.status==='FAILED') throw new Error(state.error);
      await base.System.sleep(250);
    }
  };
  api.Chat.call = options => modelRequest('chat-call',options);
  api.Chat.getMessagesRange = async (chatId, options) => {
    const start = Math.floor(options.start), end = Math.floor(options.end);
    if (start < 0 || end < start) throw new Error('range requires 0 <= start <= end');
    return command('chat-range', {chatId, start, end, order:options.order || 'asc'}).then(textResult);
  };
  const settingsActions = {
    listModelConfigs:'model-list', createModelConfig:'model-create',
    listFunctionModelConfigs:'function-list', listCharacterCards:'character-list',
    clearActiveCharacterCard:'character-clear', getSpeechServicesConfig:'speech-get',
    createCharacterCard:'character-create'
  };
  for (const [method, action] of Object.entries(settingsActions)) {
    api.SoftwareSettings[method] = value => command(action, value).then(textResult);
  }
  for (const [method, action] of Object.entries({
    updateModelConfig:'model-update', updateCharacterCard:'character-update'
  })) api.SoftwareSettings[method] = (id, updates) => command(action, {id, updates}).then(textResult);
  for (const [method, action] of Object.entries({deleteModelConfig:'model-delete',
    getCharacterCard:'character-get',deleteCharacterCard:'character-delete',
    setActiveCharacterCard:'character-active',exportCharacterCardToTavernJson:'character-export',
    getFunctionModelConfig:'function-get'
  })) api.SoftwareSettings[method] = id => command(action, {id}).then(textResult);
  api.SoftwareSettings.importCharacterCardFromTavernJson = tavernJson =>
    command('character-import', {tavernJson}).then(textResult);
  api.SoftwareSettings.setFunctionModelConfig = (functionType, configId, modelIndex) =>
    command('function-set', {functionType, configId, modelIndex}).then(textResult);
  api.SoftwareSettings.testModelConfigConnection = (configId, modelIndex) =>
    modelRequest('model-test', {configId, modelIndex});
  api.SoftwareSettings.setSpeechServicesConfig = value => modelRequest('speech-set',value);
  api.SoftwareSettings.testTtsPlayback = (text, options) =>
    modelRequest('speech-test', {text, options});
  api.SoftwareSettings.listSandboxPackages = async () => {
    const packages = await cli(['package','list']);
    const directory = await cli(['package','dir']);
    const rows = packages.map(p => ({packageName:p.name,displayName:p.displayName,
      description:p.description,isBuiltIn:p.isBuiltIn,enabledByDefault:p.enabledByDefault,
      enabled:p.enabled,imported:!p.isBuiltIn,isDisabledByUser:!p.enabled,
      toolCount:p.tools,manageMode:p.isBuiltIn?'builtin':'external'}));
    return textResult({externalPackagesPath:directory.packageDirectory,
      scriptDevGuide:'plugins/docs/package_builder/SCRIPT_DEV_GUIDE.md',
      totalCount:rows.length,builtInCount:rows.filter(p=>p.isBuiltIn).length,
      externalCount:rows.filter(p=>!p.isBuiltIn).length,
      enabledCount:rows.filter(p=>p.enabled).length,disabledCount:rows.filter(p=>!p.enabled).length,
      packages:rows,packageLoadErrors:await command('package-errors',{})});
  };
  api.SoftwareSettings.setSandboxPackageEnabled = async (packageName, enabled) => {
    const flag=String(enabled).trim().toLowerCase();
    if(['1','true','yes','y','on'].includes(flag)) enabled=true;
    else if(['0','false','no','n','off'].includes(flag)) enabled=false;
    else throw new Error('enabled must be true or false');
    const before = (await api.SoftwareSettings.listSandboxPackages()).packages.find(p=>p.packageName===packageName);
    if (!before) throw new Error('Package not loaded: '+packageName);
    await cli(['package',enabled?'enable':'disable',packageName]);
    const after = (await api.SoftwareSettings.listSandboxPackages()).packages.find(p=>p.packageName===packageName);
    return textResult({packageName,requestedEnabled:enabled,previousEnabled:before.enabled,
      currentEnabled:after.enabled,message:'Package setting saved'});
  };
  api.SoftwareSettings.executeSandboxScriptDirect = async options => {
    const sourcePath=options.source_path || '', supplied=typeof options.source_code==='string'&&options.source_code.trim().length>0;
    if(Boolean(sourcePath)===supplied) throw new Error('Exactly one of source_path or source_code is required');
    const source=supplied?options.source_code:(await api.Files.read(sourcePath)).content;
    const request=Object.assign({},options,{source_code:'const Tools = ('+__operitCreateLegacyTools.toString()+')(globalThis.Tools);\n'+source,execution_mode:supplied?'code':'script'});
    if(options.env_file_path) request.env_source=(await api.Files.read(options.env_file_path)).content;
    return modelRequest('script-run',request);
  };
  api.SoftwareSettings.restartMcpWithLogs = timeoutMs => modelRequest('mcp-restart', {timeoutMs});
  const android = async (action,payload) => {
    const Bridge = Java.type('app.operit.LegacyAndroidTools');
    const response = JSON.parse(await Bridge.execute(Java.getApplicationContext(),JSON.stringify(Object.assign({action},payload))));
    if (!response.success) throw new Error(response.error);
    return textResult(response.data);
  };
  const filePath = async (path,environment) => {
    const resolved=await android('resolveFilePath',{path,environment});
    return resolved.replace(/^\/storage\/emulated\/0(?=\/|$)/,'/sdcard').replace(/^\/storage\/self\/primary(?=\/|$)/,'/sdcard');
  };
  for(const method of ['exists','info','list','readBinary','open']) api.Files[method] = async (path,environment) => base.Files[method](await filePath(path,environment));
  api.Files.read = async value => {
    if(typeof value==='string') return base.Files.read(await filePath(value));
    const options=Object.assign({},value,{path:await filePath(value.path,value.environment)});
    delete options.environment; return base.Files.read(options);
  };
  api.Files.download = async (value,destination,environment,headers) => {
    if(typeof value==='string') return base.Files.download(value,await filePath(destination,environment),headers);
    const options=Object.assign({},value,{destination:await filePath(value.destination,value.environment)});
    delete options.environment; return base.Files.download(options);
  };
  api.Net.uploadFile = async options => base.Net.uploadFile(Object.assign({},options,{files:await Promise.all(options.files.map(async file=>Object.assign({},file,{file_path:await filePath(file.file_path)})))}));
  api.Files.readPart = async (path,start,end,environment) => base.Files.readPart(await filePath(path,environment),start,end);
  api.Files.find = async (path,pattern,options,environment) => base.Files.find(await filePath(path,environment),pattern,options);
  api.Files.create = async (path,content,environment) => base.Files.create(await filePath(path,environment),content);
  api.Files.edit = async (path,old,content,environment) => base.Files.edit(await filePath(path,environment),old,content);
  for(const method of ['grep','grepContext']) api.Files[method]=async (path,query,options) => {
    const mapped=Object.assign({},options);delete mapped.environment;
    return base.Files[method](await filePath(path,options&&options.environment),query,mapped);
  };
  api.Files.mkdir = async (path,parents,environment) => base.Files.mkdir(await filePath(path,environment),parents);
  api.Files.deleteFile = async (path,recursive,environment) => base.Files.deleteFile(await filePath(path,environment),recursive);
  api.Files.write = async (path,content,append,environment) => base.Files.write(await filePath(path,environment),content,append);
  api.Files.writeBinary = async (path,data,environment) => base.Files.writeBinary(await filePath(path,environment),data);
  api.Files.apply = async (path,type,old,newContent,environment) => base.Files.apply(await filePath(path,environment),type,old,newContent);
  api.Files.share = async (path,title,environment) => base.Files.share(await filePath(path,environment),title);
  api.Files.move = async (source,dest,environment) => base.Files.move(await filePath(source,environment),await filePath(dest,environment));
  api.Files.copy = async (source,dest,recursive,sourceEnvironment,destEnvironment) => base.Files.copy(await filePath(source,sourceEnvironment),await filePath(dest,destEnvironment),recursive);
  api.Files.zip = async (source,dest,environment,includeRoot) => base.Files.zip(await filePath(source,environment),await filePath(dest,environment),includeRoot);
  api.Files.unzip = async (source,dest,environment) => base.Files.unzip(await filePath(source,environment),await filePath(dest,environment));
  api.System.terminal = Object.create(base.System.terminal);
  api.System.terminal.create = sessionName => base.System.terminal.create(sessionName || 'legacy_operit_terminal');
  api.System.shell = command => android('shell',{command});
  api.System.intent = options => android('intent',{options});
  api.System.sendBroadcast = options => android('sendBroadcast',{options});
  api.UI.getPageInfo = () => android('getPageInfo',{});
  api.UI.captureScreenshot = () => android('captureScreenshot',{});
  api.UI.tap = (x,y) => android('tap',{x,y});
  api.UI.longPress = (x,y) => android('longPress',{x,y});
  api.UI.swipe = (startX,startY,endX,endY,duration) => android('swipe',{startX,startY,endX,endY,duration});
  api.UI.setText = (text,resourceId) => android('setText',{text,resourceId});
  api.UI.pressKey = keyCode => android('pressKey',{keyCode});
  api.UI.clickElement = (first,second,third) => {
    if (typeof first === 'object') return android('clickElement',{selector:first});
    const options = {};
    if (['resourceId','className','bounds'].includes(first) && typeof second === 'string') {
      options[first]=second; if(third !== undefined) options.index=third;
    } else {
      options[first.startsWith('[')?'bounds':'resourceId']=first;
      if(second !== undefined) options.index=second;
    }
    return android('clickElement',{selector:options});
  };
  api.UI.runSubAgent = async (intent,maxSteps,agentId,targetApp) => {
    const resolvedAgentId = agentId || 'default';
    const displayId = resolvedAgentId === 'default' ? 0 : await Java.type('app.operit.LegacyVirtualDisplays').ensure(Java.getApplicationContext(),resolvedAgentId);
    if (targetApp) await android('intent',{options:{package:targetApp,type:'activity'},displayId});
    const limit = maxSteps === undefined ? 20 : Math.max(1,Math.floor(maxSteps));
    const history = [{kind:'SYSTEM',content:'You control the current Android screen. Return only one JSON object per turn: {"action":"tap|longPress|swipe|clickElement|setText|pressKey|done","arguments":{...},"message":"..."}. Use exact coordinates or element selectors from the observed screen. Set done only when the user goal is visibly achieved. Do not invent observations.',metadata:{}},
      {kind:'USER',content:intent,metadata:{}}];
    const logs=[];
    for(let step=0;step<limit;step++) {
      const page=await android('getPageInfo',{displayId});
      history.push({kind:'USER',content:'Current screen: '+JSON.stringify(page),metadata:{}});
      const answer=await api.Chat.call({functionType:'UI_CONTROLLER',turns:history,recordTokenUsage:true});
      history.push({kind:'ASSISTANT',content:answer.text,metadata:{}});
      const decision=JSON.parse(answer.text.replace(/^```(?:json)?\s*/,'').replace(/\s*```$/,''));
      if(decision.action==='done') return textResult({functionName:'runSubAgent',providedParameters:{intent},agentId:resolvedAgentId,displayId,executionSuccess:true,executionMessage:decision.message,executionSteps:step,finalState:page});
      if(!['tap','longPress','swipe','clickElement','setText','pressKey'].includes(decision.action)) throw new Error('Invalid UI action: '+decision.action);
      const result=await android(decision.action,decision.action==='clickElement'?{selector:decision.arguments,displayId}:Object.assign({},decision.arguments,{displayId}));
      logs.push({action:decision.action,result});
      history.push({kind:'USER',content:'Action result: '+JSON.stringify(result),metadata:{}});
      await api.System.sleep(300);
    }
    return textResult({functionName:'runSubAgent',providedParameters:{intent},agentId:resolvedAgentId,displayId,executionSuccess:false,executionMessage:JSON.stringify(logs),executionError:'Maximum steps reached before goal was confirmed',executionSteps:limit});
  };
  api.FFmpeg.execute = async command => textResult(JSON.parse(await Java.type('app.operit.LegacyFfmpegBridge').execute(command)));
  api.FFmpeg.info = async () => textResult(JSON.parse(await Java.type('app.operit.LegacyFfmpegBridge').info()));
  api.FFmpeg.convert = async (input,output,options) => textResult(JSON.parse(await Java.type('app.operit.LegacyFfmpegBridge').convert(input,output,JSON.stringify(options || {}))));
  const workflow = async (action,payload) => {
    const result=JSON.parse(await base.SoftwareSettings.exec(['--json','legacy-workflow',action,JSON.stringify(payload)]));
    if(['getAll','get','create','update','patch','delete','enable','disable','import'].includes(action)) {
      const events=await base.SoftwareSettings.exec(['--json','legacy-workflow','events','{}']);
      const refreshed=JSON.parse(await Java.type('app.operit.LegacyWorkflowAndroidBridge').refresh(Java.getApplicationContext(),events));
      if(!refreshed.success) throw new Error('Workflow was saved but Android event registration failed: '+JSON.stringify(refreshed));
    }
    return textResult(result);
  };
  api.Workflow.getAll = () => workflow('getAll',{});
  for (const action of ['get','delete','enable','disable','trigger']) api.Workflow[action] = id => workflow(action,{workflow_id:id});
  api.Workflow.create = (name,description,nodes,connections,enabled) => workflow('create',{name,description,nodes,connections,enabled});
  api.Workflow.update = (id,updates) => workflow('update',Object.assign({workflow_id:id},updates));
  api.Workflow.patch = (id,patch) => workflow('patch',Object.assign({workflow_id:id},patch));
  api.Workflow.trigger = async id => {
    const execution=await workflow('trigger',{workflow_id:id});
    const deadline=Date.now()+300000;
    while(Date.now()<deadline) {
      const state=await workflow('execution',{workflow_id:id,execution_id:execution.executionId});
      if(state.status==='SUCCESS') return 'Workflow execution completed: '+execution.executionId;
      if(state.status==='FAILED') {const failed=Object.entries(state.nodes||{}).filter(([,node])=>node.status==='FAILED').map(([id,node])=>Object.assign({id},node));throw new Error(state.error || (failed.length?JSON.stringify(failed):'Workflow execution failed'));}
      await base.System.sleep(500);
    }
    throw new Error('Workflow is still running: '+execution.executionId+'; it was not cancelled or restarted.');
  };
  api.Workflow.setEnabled = (id,enabled) => api.Workflow[enabled?'enable':'disable'](id);
  return api;
}

// Workflow nodes store old built-in tool names, independently of package method names.
async function __operitLegacyWorkflowAction(params) {
  const api=__operitCreateLegacyTools(globalThis.Tools);
  const p=Object.assign({},params.payload);
  // Stored workflow parameters are strings, whereas the package Tools API is typed.
  const structured=['turns','extras','updates','options','custom_headers','custom_parameters','tts_headers','tts_response_pipeline','tts_cleaner_regexs','tts_vits_options'];
  for(const key of structured) if(typeof p[key]==='string'&&p[key].trim()!=='') p[key]=JSON.parse(p[key]);
  const booleanKeys=['record_token_usage','enable_thinking','interrupt','enabled','enable_max_context_mode','enable_summary','enable_summary_by_message_count','enable_direct_image_processing','enable_direct_audio_processing','enable_direct_video_processing','enable_google_search','enable_claude_1h_prompt_cache','enable_tool_call'];
  for(const key of Object.keys(p)) if(booleanKeys.includes(key)||key.endsWith('_enabled')) {
    const flag=String(p[key]).trim().toLowerCase();
    if(['1','true','yes','y','on'].includes(flag)) p[key]=true;
    else if(['0','false','no','n','off'].includes(flag)) p[key]=false;
    else throw new Error(key+' must be true or false');
  }
  const numberKeys=['max_tokens','temperature','top_p','top_k','presence_penalty','frequency_penalty','repetition_penalty','context_length','max_context_length','summary_token_threshold','summary_message_count_threshold','request_limit_per_minute','max_concurrent_requests','mnn_forward_type','mnn_thread_count','llama_thread_count','llama_context_size','llama_gpu_layers','tts_speech_rate','tts_pitch','speech_rate','pitch'];
  for(const key of numberKeys) if(p[key]!==undefined) {const number=Number(p[key]);if(!Number.isFinite(number))throw new Error(key+' must be numeric');p[key]=number;}
  const actions={
    trigger_workflow:()=>api.Workflow.trigger(p.workflow_id),
    execute_shell:()=>api.System.shell(p.command),execute_intent:()=>api.System.intent(p),send_broadcast:()=>api.System.sendBroadcast(p),
    get_page_info:()=>api.UI.getPageInfo(),capture_screenshot:()=>api.UI.captureScreenshot(),
    tap:()=>api.UI.tap(Number(p.x),Number(p.y)),long_press:()=>api.UI.longPress(Number(p.x),Number(p.y)),
    click_element:()=>api.UI.clickElement(p),set_input_text:()=>api.UI.setText(p.text,p.resource_id),press_key:()=>api.UI.pressKey(p.key_code),
    swipe:()=>api.UI.swipe(Number(p.start_x),Number(p.start_y),Number(p.end_x),Number(p.end_y),p.duration===undefined?undefined:Number(p.duration)),
    run_ui_subagent:()=>api.UI.runSubAgent(p.intent,p.max_steps===undefined?undefined:Number(p.max_steps),p.agent_id,p.target_app),
    ffmpeg_execute:()=>api.FFmpeg.execute(p.command),ffmpeg_info:()=>api.FFmpeg.info(),ffmpeg_convert:()=>api.FFmpeg.convert(p.input_path,p.output_path,p),
    call_chat_model:()=>api.Chat.call({functionType:p.function_type,turns:p.turns,recordTokenUsage:p.record_token_usage,enableThinking:p.enable_thinking}),get_chat_messages_range:()=>api.Chat.getMessagesRange(p.chat_id,{start:Number(p.start),end:Number(p.end),order:p.order}),
    list_sandbox_packages:()=>api.SoftwareSettings.listSandboxPackages(),set_sandbox_package_enabled:()=>api.SoftwareSettings.setSandboxPackageEnabled(p.package_name,p.enabled===true||p.enabled==='true'),
    execute_sandbox_script_direct:()=>api.SoftwareSettings.executeSandboxScriptDirect(p),restart_mcp_with_logs:()=>api.SoftwareSettings.restartMcpWithLogs(p.timeout_ms),
    get_speech_services_config:()=>api.SoftwareSettings.getSpeechServicesConfig(),set_speech_services_config:()=>api.SoftwareSettings.setSpeechServicesConfig(p),test_tts_playback:()=>api.SoftwareSettings.testTtsPlayback(p.text,p),
    list_model_configs:()=>api.SoftwareSettings.listModelConfigs(),create_model_config:()=>api.SoftwareSettings.createModelConfig(p),update_model_config:()=>api.SoftwareSettings.updateModelConfig(p.config_id,p.updates||Object.fromEntries(Object.entries(p).filter(([key])=>key!=='config_id'))),delete_model_config:()=>api.SoftwareSettings.deleteModelConfig(p.config_id),
    list_function_model_configs:()=>api.SoftwareSettings.listFunctionModelConfigs(),get_function_model_config:()=>api.SoftwareSettings.getFunctionModelConfig(p.function_type),set_function_model_config:()=>api.SoftwareSettings.setFunctionModelConfig(p.function_type,p.config_id,p.model_index===undefined?undefined:Number(p.model_index)),test_model_config_connection:()=>api.SoftwareSettings.testModelConfigConnection(p.config_id,p.model_index===undefined?undefined:Number(p.model_index))
  };
  if(!Object.hasOwn(actions,params.action)) throw new Error('Unknown legacy workflow action: '+params.action);
  return actions[params.action]();
}
