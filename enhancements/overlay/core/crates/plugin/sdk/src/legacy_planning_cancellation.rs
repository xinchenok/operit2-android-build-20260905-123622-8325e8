//! Cancellation shared by the enhanced legacy planner and its owning ToolPkg execution.
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use std::sync::atomic::{AtomicBool, Ordering};
struct Request { parent:String, cancelled:Arc<AtomicBool> }
fn requests()->&'static Mutex<HashMap<String,Request>> {
    static REQUESTS:OnceLock<Mutex<HashMap<String,Request>>>=OnceLock::new();
    REQUESTS.get_or_init(||Mutex::new(HashMap::new()))
}
pub fn register(id:&str,parent:&str)->Result<Arc<AtomicBool>,String>{
    let mut requests=requests().lock().map_err(|e|e.to_string())?;
    if requests.contains_key(id){return Err(format!("Planning request already exists: {id}"));}
    let cancelled=Arc::new(AtomicBool::new(false));
    requests.insert(id.into(),Request{parent:parent.into(),cancelled:cancelled.clone()});Ok(cancelled)
}
pub fn remove(id:&str){if let Ok(mut requests)=requests().lock(){requests.remove(id);}}
pub fn cancel(id:&str)->bool {
    if let Ok(requests)=requests().lock(){if let Some(request)=requests.get(id){request.cancelled.store(true,Ordering::Release);return true;}}false
}
pub fn cancel_parent(parent:&str){
    if parent.is_empty(){return;}
    if let Ok(requests)=requests().lock(){for request in requests.values().filter(|r|r.parent==parent){request.cancelled.store(true,Ordering::Release);}}
}
