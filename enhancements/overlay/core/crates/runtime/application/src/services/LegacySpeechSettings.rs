//! Enhanced Operit1 speech options missing from the base profile schema.
use crate::data::preferences::EnvPreferences::EnvPreferences;
use serde_json::{json,Value};
const KEY:&str="OPERIT2_LEGACY_SPEECH_OPTIONS";
pub fn read()->Result<Value,String>{
    match EnvPreferences::getInstance().getEnv(KEY).map_err(|e|e.to_string())? {
        Some(value)=>serde_json::from_str(&value).map_err(|e|e.to_string()),
        None=>Ok(json!({"tts_pitch":1.0,"tts_cleaner_regexs":[]}))
    }
}
pub fn update(updates:&Value)->Result<(),String>{
    let mut value=read()?;
    if let Some(pitch)=updates.get("tts_pitch"){
        let number=pitch.as_f64().filter(|n|n.is_finite()&&*n>0.0).ok_or("TTS pitch must be positive")?;value["tts_pitch"]=json!(number);
    }
    if let Some(regexes)=updates.get("tts_cleaner_regexs"){
        let regexes=match regexes{Value::String(s)=>serde_json::from_str(s).map_err(|e|e.to_string())?,_=>regexes.clone()};
        for pattern in regexes.as_array().ok_or("TTS cleaner patterns must be an array")?{regex::Regex::new(pattern.as_str().ok_or("TTS cleaner pattern must be string")?).map_err(|e|e.to_string())?;}
        value["tts_cleaner_regexs"]=regexes;
    }
    EnvPreferences::getInstance().setEnv(KEY,&value.to_string()).map_err(|e|e.to_string())
}
pub fn clean(text:&str)->Result<String,String>{
    let settings=read()?;let mut text=text.to_string();
    for pattern in settings["tts_cleaner_regexs"].as_array().ok_or("Invalid stored TTS cleaner list")?{
        text=regex::Regex::new(pattern.as_str().ok_or("Invalid stored TTS cleaner pattern")?).map_err(|e|e.to_string())?.replace_all(&text,"").into_owned();
    }
    Ok(text.trim().to_string())
}
pub fn pitch()->Result<f64,String>{read()?["tts_pitch"].as_f64().ok_or_else(||"Invalid stored TTS pitch".to_string())}
