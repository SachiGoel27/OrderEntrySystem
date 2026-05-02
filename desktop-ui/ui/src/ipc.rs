use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_wasm_bindgen::{from_value, to_value};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

pub async fn invoke_json<T: DeserializeOwned>(cmd: &str, args: &impl Serialize) -> Result<T, String> {
    let window = web_sys::window().ok_or_else(|| "no window".to_string())?;
    let invoke = js_sys::Reflect::get(&window, &JsValue::from_str("__OES_INVOKE__"))
        .map_err(|_| "missing __OES_INVOKE__ (index.html script)".to_string())?;
    let func: js_sys::Function = invoke
        .dyn_into()
        .map_err(|_| "__OES_INVOKE__ is not a function".to_string())?;
    let args_val = to_value(args).map_err(|e| e.to_string())?;
    let promise = func
        .call2(&JsValue::NULL, &JsValue::from_str(cmd), &args_val)
        .map_err(|e| format!("invoke call failed: {:?}", e))?;
    let promise = js_sys::Promise::resolve(&promise);
    let result = JsFuture::from(promise)
        .await
        .map_err(|e| format!("invoke await failed: {:?}", e))?;
    from_value(result).map_err(|e| e.to_string())
}
