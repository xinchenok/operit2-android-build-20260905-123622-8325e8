// Operit1 contracts over Operit2 executors. This factory never modifies native Tools.
function __operitCreateLegacyTools(base) {
  const api = Object.create(base);
  for (const name of ['Chat', 'Files', 'SoftwareSettings', 'System', 'UI', 'FFmpeg', 'Workflow']) {
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
    setSpeechServicesConfig:'speech-set', createCharacterCard:'character-create'
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
    command('model-test', {configId, modelIndex}).then(textResult);
  api.SoftwareSettings.testTtsPlayback = (text, options) =>
    command('speech-test', {text, options}).then(textResult);
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
    const before = (await api.SoftwareSettings.listSandboxPackages()).packages.find(p=>p.packageName===packageName);
    if (!before) throw new Error('Package not loaded: '+packageName);
    await cli(['package',enabled?'enable':'disable',packageName]);
    const after = (await api.SoftwareSettings.listSandboxPackages()).packages.find(p=>p.packageName===packageName);
    return textResult({packageName,requestedEnabled:enabled,previousEnabled:before.enabled,
      currentEnabled:after.enabled,message:'Package setting saved'});
  };
  api.SoftwareSettings.executeSandboxScriptDirect = options => command('script-run', options).then(textResult);
  api.SoftwareSettings.restartMcpWithLogs = timeoutMs => command('mcp-restart', {timeoutMs}).then(textResult);
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
  const workflow = (action,payload) => base.SoftwareSettings.exec(['--json','legacy-workflow',action,JSON.stringify(payload)]).then(JSON.parse).then(textResult);
  api.Workflow.getAll = () => workflow('getAll',{});
  for (const action of ['get','delete','enable','disable','trigger']) api.Workflow[action] = id => workflow(action,{workflow_id:id});
  api.Workflow.create = (name,description,nodes,connections,enabled) => workflow('create',{name,description,nodes,connections,enabled});
  api.Workflow.update = (id,updates) => workflow('update',Object.assign({workflow_id:id},updates));
  api.Workflow.patch = (id,patch) => workflow('patch',Object.assign({workflow_id:id},patch));
  api.Workflow.trigger = async id => {
    const execution=await workflow('trigger',{workflow_id:id});
    while(true) {
      const state=await workflow('execution',{workflow_id:id,execution_id:execution.executionId});
      if(state.status==='SUCCESS') return 'Workflow execution completed: '+execution.executionId;
      if(state.status==='FAILED') throw new Error(state.error || 'Workflow execution failed');
      await base.System.sleep(500);
    }
  };
  api.Workflow.setEnabled = (id,enabled) => api.Workflow[enabled?'enable':'disable'](id);
  return api;
}

// Workflow nodes store old built-in tool names, independently of package method names.
async function __operitLegacyWorkflowAction(params) {
  const api=__operitCreateLegacyTools(globalThis.Tools);
  const p=params.payload;
  const actions={
    execute_shell:()=>api.System.shell(p.command),execute_intent:()=>api.System.intent(p),send_broadcast:()=>api.System.sendBroadcast(p),
    get_page_info:()=>api.UI.getPageInfo(),capture_screenshot:()=>api.UI.captureScreenshot(),
    tap:()=>api.UI.tap(Number(p.x),Number(p.y)),long_press:()=>api.UI.longPress(Number(p.x),Number(p.y)),
    click_element:()=>api.UI.clickElement(p),set_input_text:()=>api.UI.setText(p.text,p.resource_id),press_key:()=>api.UI.pressKey(p.key_code),
    swipe:()=>api.UI.swipe(Number(p.start_x),Number(p.start_y),Number(p.end_x),Number(p.end_y),p.duration===undefined?undefined:Number(p.duration)),
    run_ui_subagent:()=>api.UI.runSubAgent(p.intent,p.max_steps===undefined?undefined:Number(p.max_steps),p.agent_id,p.target_app),
    ffmpeg_execute:()=>api.FFmpeg.execute(p.command),ffmpeg_info:()=>api.FFmpeg.info(),ffmpeg_convert:()=>api.FFmpeg.convert(p.input_path,p.output_path,p),
    call_chat_model:()=>api.Chat.call(p),get_chat_messages_range:()=>api.Chat.getMessagesRange(p.chat_id,{start:Number(p.start),end:Number(p.end),order:p.order}),
    list_sandbox_packages:()=>api.SoftwareSettings.listSandboxPackages(),set_sandbox_package_enabled:()=>api.SoftwareSettings.setSandboxPackageEnabled(p.package_name,p.enabled===true||p.enabled==='true'),
    execute_sandbox_script_direct:()=>api.SoftwareSettings.executeSandboxScriptDirect(p),restart_mcp_with_logs:()=>api.SoftwareSettings.restartMcpWithLogs(p.timeout_ms),
    get_speech_services_config:()=>api.SoftwareSettings.getSpeechServicesConfig(),set_speech_services_config:()=>api.SoftwareSettings.setSpeechServicesConfig(p),test_tts_playback:()=>api.SoftwareSettings.testTtsPlayback(p.text,p),
    list_model_configs:()=>api.SoftwareSettings.listModelConfigs(),create_model_config:()=>api.SoftwareSettings.createModelConfig(p),update_model_config:()=>api.SoftwareSettings.updateModelConfig(p.config_id,p.updates||p),delete_model_config:()=>api.SoftwareSettings.deleteModelConfig(p.config_id),
    list_function_model_configs:()=>api.SoftwareSettings.listFunctionModelConfigs(),get_function_model_config:()=>api.SoftwareSettings.getFunctionModelConfig(p.function_type),set_function_model_config:()=>api.SoftwareSettings.setFunctionModelConfig(p.function_type,p.config_id,p.model_index===undefined?undefined:Number(p.model_index)),test_model_config_connection:()=>api.SoftwareSettings.testModelConfigConnection(p.config_id,p.model_index===undefined?undefined:Number(p.model_index))
  };
  if(!Object.hasOwn(actions,params.action)) throw new Error('Unknown legacy workflow action: '+params.action);
  return actions[params.action]();
}
