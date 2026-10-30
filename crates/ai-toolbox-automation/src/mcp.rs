//! Bounded, synchronous stdio MCP adapter for one shared path service.
//! Explicitly supports revision 2025-11-25, not an implicit latest revision.
//! Discovery never initializes an owner, storage or a desktop.
use serde_json::{json, Value};
use std::io::{self, BufRead, Write};

pub const PROTOCOL_VERSION: &str = "2025-11-25";
pub const TOOL_NAME: &str = "backup_normalize_path";
pub const MAX_MESSAGE_BYTES: usize = 1024 * 1024;

#[derive(Default)]
pub struct Server { initialized: bool, ready: bool }
fn error(id: Value, code: i32, message: &'static str) -> Value {
    json!({"jsonrpc":"2.0","id":id,"error":{"code":code,"message":message}})
}
fn success(id: Value, result: Value) -> Value { json!({"jsonrpc":"2.0","id":id,"result":result}) }
fn empty_params(params: Option<&Value>) -> bool {
    params.is_none_or(|p| p.as_object().is_some_and(|o| o.keys().all(|k| k == "_meta")))
}
fn valid_id(id: &Value) -> bool { id.is_string() || id.as_i64().is_some() || id.as_u64().is_some() }
impl Server {
    pub fn handle(&mut self, bytes: &[u8], normalize: &mut impl FnMut(&str) -> Result<String, ai_toolbox_paths::PathRequestError>) -> Option<Value> {
        if bytes.len() > MAX_MESSAGE_BYTES { return Some(error(Value::Null,-32600,"Message too large")); }
        let request: Value = match serde_json::from_slice(bytes) {
            Ok(v) => v, Err(_) => return Some(error(Value::Null,-32700,"Parse error")),
        };
        let Some(object) = request.as_object() else { return Some(error(Value::Null,-32600,"Invalid request")); };
        let id = object.get("id");
        let method = object.get("method").and_then(Value::as_str);
        // Recognizable notifications never receive responses or invoke tools.
        // Invalid initialized params must not advance the handshake.
        if id.is_none() && method.is_some() && object.get("jsonrpc").and_then(Value::as_str) == Some("2.0") {
            if method == Some("notifications/initialized") && self.initialized && empty_params(object.get("params")) { self.ready = true; }
            return None;
        }
        if object.get("jsonrpc").and_then(Value::as_str) != Some("2.0") || method.is_none() || id.is_some_and(|v| !valid_id(v)) || object.get("params").is_some_and(|v| !v.is_object()) {
            return Some(error(id.filter(|v| valid_id(v)).cloned().unwrap_or(Value::Null),-32600,"Invalid request"));
        }
        let Some(id) = id.cloned() else { return None; };
        let method = method.unwrap();
        let params = object.get("params");
        if method == "initialize" {
            if self.initialized { return Some(error(id,-32600,"Already initialized")); }
            let valid = params.is_some_and(|p| p.get("protocolVersion").and_then(Value::as_str).is_some() && p.get("capabilities").is_some_and(Value::is_object) && p.get("clientInfo").is_some_and(|c| c.get("name").and_then(Value::as_str).is_some() && c.get("version").and_then(Value::as_str).is_some()));
            if !valid { return Some(error(id,-32602,"Invalid params")); }
            self.initialized = true;
            return Some(success(id,json!({"protocolVersion":PROTOCOL_VERSION,"capabilities":{"tools":{"listChanged":false}},"serverInfo":{"name":"ai-toolbox-cli","version":env!("CARGO_PKG_VERSION")},"instructions":"Only backup path normalization is implemented. No account, storage or desktop operation is exposed."})));
        }
        if method == "ping" { return Some(if empty_params(params) { success(id,json!({})) } else { error(id,-32602,"Invalid params") }); }
        if !self.ready { return Some(error(id,-32600,"Initialization required")); }
        match method {
            "tools/list" => {
                if !empty_params(params) { return Some(error(id,-32602,"Invalid params")); }
                Some(success(id,json!({"tools":[{"name":TOOL_NAME,"description":"Normalize a supplied backup path through the desktop's shared service. Resolves host home/config directories and path environment, without reading or writing target file contents. This is not a path safety validator.","inputSchema":{"type":"object","properties":{"path":{"type":"string"}},"required":["path"],"additionalProperties":false},"outputSchema":{"type":"object","properties":{"path":{"type":"string"}},"required":["path"],"additionalProperties":false},"annotations":{"readOnlyHint":true,"destructiveHint":false,"idempotentHint":true,"openWorldHint":false}}]})))
            }
            "tools/call" => {
                let Some(params) = params.and_then(Value::as_object) else { return Some(error(id,-32602,"Invalid params")); };
                if params.keys().any(|k| !matches!(k.as_str(),"name"|"arguments"|"_meta")) || params.get("name").and_then(Value::as_str) != Some(TOOL_NAME) { return Some(error(id,-32602,"Invalid params")); }
                let Some(args) = params.get("arguments").and_then(Value::as_object) else { return Some(error(id,-32602,"Invalid params")); };
                let Some(path) = args.get("path").and_then(Value::as_str) else { return Some(error(id,-32602,"Invalid params")); };
                if args.len() != 1 || ai_toolbox_paths::validate_backup_path_input(path).is_err() { return Some(error(id,-32602,"Invalid params")); }
                match normalize(path) {
                    Ok(path) => { let output = json!({"path":path}); Some(success(id,json!({"content":[{"type":"text","text":output.to_string()}],"structuredContent":output,"isError":false}))) },
                    Err(error) => Some(success(id,json!({"content":[{"type":"text","text":error.message()}],"isError":true}))),
                }
            }
            _ => Some(error(id,-32601,"Method not found")),
        }
    }
}
/// Serial execution gives output backpressure without an unbounded work queue.
/// Overlong lines close the transport without draining unbounded input. EOF
/// closes it too; an unterminated final message is invalid, never dispatched.
pub fn serve<R: BufRead,W: Write>(mut input: R, mut output: W, mut normalize: impl FnMut(&str)->Result<String,ai_toolbox_paths::PathRequestError>) -> io::Result<()> {
    let mut server = Server::default();
    loop {
        let mut message = Vec::new();
        let mut complete = false;
        loop {
            let available = input.fill_buf()?;
            if available.is_empty() { break; }
            let count = available.iter().position(|b| *b == b'\n').map_or(available.len(),|p| p+1);
            if message.len().saturating_add(count) > MAX_MESSAGE_BYTES {
                write_response(&mut output,&error(Value::Null,-32600,"Message too large"))?;
                return Err(io::Error::new(io::ErrorKind::InvalidData,"MCP message too large"));
            }
            complete = available[count-1] == b'\n';
            message.extend_from_slice(&available[..count]);
            input.consume(count);
            if complete { break; }
        }
        if message.is_empty() { return Ok(()); }
        if !complete {
            write_response(&mut output,&error(Value::Null,-32700,"Unterminated message"))?;
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof,"Unterminated MCP message"));
        }
        if let Some(response) = server.handle(&message,&mut normalize) { write_response(&mut output,&response)?; }
    }
}
fn write_response(out: &mut impl Write, response: &Value) -> io::Result<()> {
    serde_json::to_writer(&mut *out,response)?;
    out.write_all(b"\n")?; out.flush()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn init(s: &mut Server) {
        assert_eq!(s.handle(br#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"fixture","version":"1"}}}"#,&mut |_| panic!()).unwrap()["result"]["protocolVersion"],PROTOCOL_VERSION);
        assert!(s.handle(br#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,&mut |_| panic!()).is_none());
    }
    #[test] fn handshake_and_discovery_never_dispatch() {
        let mut s = Server::default();
        assert_eq!(s.handle(br#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#,&mut |_| panic!()).unwrap()["error"]["code"],-32600);
        init(&mut s);
        let list=s.handle(br#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#,&mut |_| panic!()).unwrap();
        assert_eq!(list["result"]["tools"].as_array().unwrap().len(),1);
        assert_eq!(list["result"]["tools"][0]["name"],TOOL_NAME);
    }
    #[test] fn notifications_and_invalid_calls_do_not_dispatch() {
        let mut s=Server::default(); init(&mut s);
        for bytes in [br#"{"jsonrpc":"2.0","method":"tools/call","params":{"name":"backup_normalize_path","arguments":{"path":"x"}}}"#.as_slice(),br#"{"jsonrpc":"2.0","method":"notifications/cancelled","params":[]}"#] {
            assert!(s.handle(bytes,&mut |_| panic!()).is_none());
        }
        for params in [json!({"name":"unknown","arguments":{"path":"x"}}), json!({"name":TOOL_NAME,"arguments":{"path":"x","extra":true}}),json!({"name":TOOL_NAME,"arguments":{"path":"x".repeat(crate::MAX_PATH_BYTES+1)}})] {
            let req=json!({"jsonrpc":"2.0","id":"t","method":"tools/call","params":params});
            assert_eq!(s.handle(&serde_json::to_vec(&req).unwrap(),&mut |_| panic!()).unwrap()["error"]["code"],-32602);
        }
    }
    #[test] fn malformed_initialized_does_not_make_ready() {
        let mut s=Server {initialized:true,ready:false};
        assert!(s.handle(br#"{"jsonrpc":"2.0","method":"notifications/initialized","params":[]}"#,&mut |_| panic!()).is_none());
        assert!(!s.ready);
    }
    #[test] fn tool_result_uses_service_and_preserves_typed_failure() {
        let mut s=Server::default(); init(&mut s);
        let req=br#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"backup_normalize_path","arguments":{"path":"../fixture"}}}"#;
        let r=s.handle(req,&mut |p| {assert_eq!(p,"../fixture");Ok(p.into())}).unwrap();
        assert_eq!(r["result"]["structuredContent"]["path"],"../fixture");
        assert_eq!(r["result"]["isError"],false);
        assert_eq!(s.handle(req,&mut |_| Err(ai_toolbox_paths::PathRequestError::ExpansionLimitExceeded)).unwrap()["result"]["isError"],true);
    }
    #[test] fn frame_limits_and_final_newline_are_enforced_before_dispatch() {
        for bytes in [vec![b'x';MAX_MESSAGE_BYTES+1],b"{}".to_vec()] {
            let mut out=Vec::new();
            assert!(serve(std::io::Cursor::new(bytes),&mut out, |_| panic!()).is_err());
            assert_eq!(out.iter().filter(|b| **b==b'\n').count(),1);
        }
    }
}
