use mlua::prelude::*;
use std::collections::HashMap;
use tracing::debug;

/// Register the Ando PDK (Plugin Development Kit) into a Lua VM.
///
/// This creates the `ando` global table with sub-modules:
/// - `ando.request` — Access request data
/// - `ando.response` — Modify response data
/// - `ando.log` — Logging functions
/// - `ando.ctx` — Per-request context variables
/// - `ando.var` — Nginx-like variables
///
/// This mirrors APISIX's PDK API so that plugin authors have a familiar interface.
pub fn register_pdk(lua: &Lua) -> anyhow::Result<()> {
    let globals = lua.globals();

    // Create the main `ando` PDK table
    let ando = lua.create_table()?;

    // -- ando.request --
    let request = lua.create_table()?;

    request.set(
        "get_method",
        lua.create_function(|lua, ()| {
            let ctx = get_ctx(lua)?;
            let method: String = ctx.get("method")?;
            Ok(method)
        })?,
    )?;

    request.set(
        "get_uri",
        lua.create_function(|lua, ()| {
            let ctx = get_ctx(lua)?;
            let uri: String = ctx.get("uri")?;
            Ok(uri)
        })?,
    )?;

    request.set(
        "get_path",
        lua.create_function(|lua, ()| {
            let ctx = get_ctx(lua)?;
            let path: String = ctx.get("path")?;
            Ok(path)
        })?,
    )?;

    request.set(
        "get_query",
        lua.create_function(|lua, ()| {
            let ctx = get_ctx(lua)?;
            let query: String = ctx.get("query")?;
            Ok(query)
        })?,
    )?;

    request.set(
        "get_header",
        lua.create_function(|lua, name: String| {
            let ctx = get_ctx(lua)?;
            let headers: LuaTable = ctx.get("headers")?;
            let value: Option<String> = headers.get(name.to_lowercase())?;
            Ok(value)
        })?,
    )?;

    request.set(
        "get_headers",
        lua.create_function(|lua, ()| {
            let ctx = get_ctx(lua)?;
            let headers: LuaTable = ctx.get("headers")?;
            Ok(headers)
        })?,
    )?;

    request.set(
        "set_header",
        lua.create_function(|lua, (name, value): (String, String)| {
            let ctx = get_ctx(lua)?;
            let headers: LuaTable = ctx.get("headers")?;
            headers.set(name.to_lowercase(), value)?;
            Ok(())
        })?,
    )?;

    request.set(
        "get_body",
        lua.create_function(|lua, ()| {
            let ctx = get_ctx(lua)?;
            let body: Option<String> = ctx.get("body")?;
            Ok(body)
        })?,
    )?;

    request.set(
        "get_remote_addr",
        lua.create_function(|lua, ()| {
            let ctx = get_ctx(lua)?;
            let ip: String = ctx.get("client_ip")?;
            Ok(ip)
        })?,
    )?;

    request.set(
        "get_uri_args",
        lua.create_function(|lua, ()| {
            let ctx = get_ctx(lua)?;
            let query: String = ctx.get("query")?;
            let args = lua.create_table()?;
            for pair in query.split('&') {
                if let Some((k, v)) = pair.split_once('=') {
                    args.set(k.to_string(), v.to_string())?;
                }
            }
            Ok(args)
        })?,
    )?;

    ando.set("request", request)?;

    // -- ando.response --
    let response = lua.create_table()?;

    response.set(
        "set_status",
        lua.create_function(|lua, status: u16| {
            let resp = get_or_create_response(lua)?;
            resp.set("status", status)?;
            Ok(())
        })?,
    )?;

    response.set(
        "set_header",
        lua.create_function(|lua, (name, value): (String, String)| {
            let resp = get_or_create_response(lua)?;
            let headers: LuaTable = resp
                .get::<LuaTable>("headers")
                .unwrap_or_else(|_| lua.create_table().unwrap());
            headers.set(name, value)?;
            resp.set("headers", headers)?;
            Ok(())
        })?,
    )?;

    response.set(
        "set_body",
        lua.create_function(|lua, body: String| {
            let resp = get_or_create_response(lua)?;
            resp.set("body", body)?;
            Ok(())
        })?,
    )?;

    response.set(
        "exit",
        lua.create_function(|lua, (status, body): (u16, Option<String>)| {
            let resp = get_or_create_response(lua)?;
            resp.set("status", status)?;
            resp.set("exit", true)?;
            if let Some(b) = body {
                resp.set("body", b)?;
            }
            Ok(())
        })?,
    )?;

    ando.set("response", response)?;

    // -- ando.log --
    let log = lua.create_table()?;

    log.set(
        "info",
        lua.create_function(|_lua, msg: String| {
            tracing::info!(lua_plugin = true, "{}", msg);
            Ok(())
        })?,
    )?;

    log.set(
        "warn",
        lua.create_function(|_lua, msg: String| {
            tracing::warn!(lua_plugin = true, "{}", msg);
            Ok(())
        })?,
    )?;

    log.set(
        "error",
        lua.create_function(|_lua, msg: String| {
            tracing::error!(lua_plugin = true, "{}", msg);
            Ok(())
        })?,
    )?;

    log.set(
        "debug",
        lua.create_function(|_lua, msg: String| {
            tracing::debug!(lua_plugin = true, "{}", msg);
            Ok(())
        })?,
    )?;

    ando.set("log", log)?;

    // -- ando.ctx --
    let ctx_module = lua.create_table()?;

    ctx_module.set(
        "get",
        lua.create_function(|lua, key: String| {
            let ctx = get_ctx(lua)?;
            let vars: LuaTable = ctx.get("vars")?;
            let value: LuaValue = vars.get(key)?;
            Ok(value)
        })?,
    )?;

    ctx_module.set(
        "set",
        lua.create_function(|lua, (key, value): (String, LuaValue)| {
            let ctx = get_ctx(lua)?;
            let vars: LuaTable = ctx.get("vars")?;
            vars.set(key, value)?;
            Ok(())
        })?,
    )?;

    ando.set("ctx", ctx_module)?;

    // -- ando.plugin --
    let plugin_module = lua.create_table()?;

    // Helper to define plugin metadata
    plugin_module.set(
        "define",
        lua.create_function(|lua, config: LuaTable| {
            // Store plugin definition for the runtime to read
            lua.globals().set("__ando_plugin_def", config)?;
            Ok(())
        })?,
    )?;

    ando.set("plugin", plugin_module)?;

    // -- ando.json --
    let json = lua.create_table()?;

    json.set(
        "encode",
        lua.create_function(|_lua, value: LuaValue| {
            let json_str = serde_json::to_string(&lua_value_to_json(value))
                .map_err(|e| LuaError::RuntimeError(format!("JSON encode error: {}", e)))?;
            Ok(json_str)
        })?,
    )?;

    json.set(
        "decode",
        lua.create_function(|lua, s: String| {
            let value: serde_json::Value = serde_json::from_str(&s)
                .map_err(|e| LuaError::RuntimeError(format!("JSON decode error: {}", e)))?;
            json_to_lua_value(lua, &value)
        })?,
    )?;

    ando.set("json", json)?;

    // Set the global `ando` table
    globals.set("ando", ando)?;

    debug!("PDK registered in Lua VM");
    Ok(())
}

/// Get the per-request context table from the Lua global.
fn get_ctx(lua: &Lua) -> LuaResult<LuaTable> {
    lua.globals()
        .get::<LuaTable>("__ando_ctx")
        .map_err(|_| LuaError::RuntimeError("Request context not initialized".into()))
}

/// Get or create the response table.
fn get_or_create_response(lua: &Lua) -> LuaResult<LuaTable> {
    let globals = lua.globals();
    match globals.get::<LuaTable>("__ando_response") {
        Ok(resp) => Ok(resp),
        Err(_) => {
            let resp = lua.create_table()?;
            let headers = lua.create_table()?;
            resp.set("headers", headers)?;
            resp.set("exit", false)?;
            globals.set("__ando_response", resp.clone())?;
            Ok(resp)
        }
    }
}

/// Convert a Lua value to serde_json::Value.
fn lua_value_to_json(value: LuaValue) -> serde_json::Value {
    match value {
        LuaValue::Nil => serde_json::Value::Null,
        LuaValue::Boolean(b) => serde_json::Value::Bool(b),
        LuaValue::Integer(i) => serde_json::json!(i),
        LuaValue::Number(n) => serde_json::json!(n),
        LuaValue::String(s) => serde_json::Value::String(s.to_string_lossy().to_string()),
        LuaValue::Table(t) => {
            // Detect if table is array or object
            let len = t.raw_len();
            if len > 0 {
                let mut arr = Vec::new();
                for i in 1..=len {
                    if let Ok(v) = t.raw_get::<LuaValue>(i) {
                        arr.push(lua_value_to_json(v));
                    }
                }
                serde_json::Value::Array(arr)
            } else {
                let mut map = serde_json::Map::new();
                if let Ok(pairs) = t.pairs::<String, LuaValue>().collect::<Result<Vec<_>, _>>() {
                    for (k, v) in pairs {
                        map.insert(k, lua_value_to_json(v));
                    }
                }
                serde_json::Value::Object(map)
            }
        }
        _ => serde_json::Value::Null,
    }
}

/// Convert serde_json::Value to Lua value.
pub fn json_to_lua_value(lua: &Lua, value: &serde_json::Value) -> LuaResult<LuaValue> {
    match value {
        serde_json::Value::Null => Ok(LuaValue::Nil),
        serde_json::Value::Bool(b) => Ok(LuaValue::Boolean(*b)),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(LuaValue::Integer(i))
            } else if let Some(f) = n.as_f64() {
                Ok(LuaValue::Number(f))
            } else {
                Ok(LuaValue::Nil)
            }
        }
        serde_json::Value::String(s) => {
            let ls = lua.create_string(s)?;
            Ok(LuaValue::String(ls))
        }
        serde_json::Value::Array(arr) => {
            let table = lua.create_table()?;
            for (i, v) in arr.iter().enumerate() {
                table.set(i + 1, json_to_lua_value(lua, v)?)?;
            }
            Ok(LuaValue::Table(table))
        }
        serde_json::Value::Object(map) => {
            let table = lua.create_table()?;
            for (k, v) in map {
                table.set(k.as_str(), json_to_lua_value(lua, v)?)?;
            }
            Ok(LuaValue::Table(table))
        }
    }
}

/// Set up the per-request context in the Lua VM.
pub fn setup_request_context(
    lua: &Lua,
    method: &str,
    uri: &str,
    path: &str,
    query: &str,
    headers: &HashMap<String, String>,
    client_ip: &str,
    body: Option<&[u8]>,
    vars: &HashMap<String, serde_json::Value>,
) -> LuaResult<()> {
    let ctx = lua.create_table()?;
    ctx.set("method", method)?;
    ctx.set("uri", uri)?;
    ctx.set("path", path)?;
    ctx.set("query", query)?;
    ctx.set("client_ip", client_ip)?;

    // Request headers
    let h = lua.create_table()?;
    for (k, v) in headers {
        h.set(k.to_lowercase().as_str(), v.as_str())?;
    }
    ctx.set("headers", h)?;

    // Request body
    if let Some(body) = body {
        if let Ok(s) = std::str::from_utf8(body) {
            ctx.set("body", s)?;
        }
    }

    // Context variables
    let lua_vars = lua.create_table()?;
    for (k, v) in vars {
        lua_vars.set(k.as_str(), json_to_lua_value(lua, v)?)?;
    }
    ctx.set("vars", lua_vars)?;

    lua.globals().set("__ando_ctx", ctx)?;

    // Reset response state
    lua.globals().set("__ando_response", LuaValue::Nil)?;

    Ok(())
}

/// Read the response state from the Lua VM after plugin execution.
pub fn read_response_state(lua: &Lua) -> LuaResult<Option<LuaResponseState>> {
    let globals = lua.globals();
    let resp: Option<LuaTable> = globals.get("__ando_response").ok();

    if let Some(resp) = resp {
        let exit: bool = resp.get("exit").unwrap_or(false);
        if !exit {
            return Ok(None);
        }

        let status: u16 = resp.get("status").unwrap_or(200);
        let body: Option<String> = resp.get("body").ok();

        let mut headers = HashMap::new();
        if let Ok(h) = resp.get::<LuaTable>("headers") {
            if let Ok(pairs) = h.pairs::<String, String>().collect::<Result<Vec<_>, _>>() {
                for (k, v) in pairs {
                    headers.insert(k, v);
                }
            }
        }

        Ok(Some(LuaResponseState {
            status,
            headers,
            body,
        }))
    } else {
        Ok(None)
    }
}

/// Response state extracted from Lua VM.
#[derive(Debug)]
pub struct LuaResponseState {
    pub status: u16,
    pub headers: HashMap<String, String>,
    pub body: Option<String>,
}
