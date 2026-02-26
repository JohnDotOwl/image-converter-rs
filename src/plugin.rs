/// JSON-RPC 2.0 plugin for ClippyBot Mainframe.
///
/// Reads JSON-RPC requests from stdin (one per line), dispatches to the
/// image-converter-rs library, and writes JSON-RPC responses to stdout.
/// All diagnostic output goes to stderr.
use image_converter_rs::{ConvertOptions, ResizeMode, ResizeOptions, convert_directory, convert_image_file};
use serde_json::{Value, json};
use std::io::{self, BufRead, Write};
use std::path::Path;

const NAME: &str = "image-converter";
const VERSION: &str = "0.1.0";
const PROTOCOL_VERSION: &str = "2024-11-05";

fn main() {
    let stdin = io::stdin().lock();
    let mut stdout = io::stdout().lock();

    for line in stdin.lines() {
        let line = match line {
            Ok(l) => l,
            Err(e) => {
                log("error", &format!("stdin read error: {e}"));
                break;
            }
        };
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let req: Value = match serde_json::from_str(trimmed) {
            Ok(v) => v,
            Err(e) => {
                log("error", &format!("JSON parse error: {e}"));
                continue;
            }
        };

        let id = req.get("id").cloned();
        // Notifications (no id) — nothing to respond to
        if id.is_none() || id.as_ref().is_some_and(Value::is_null) {
            continue;
        }
        let id = id.unwrap();

        let method = req
            .get("method")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let params = req
            .get("params")
            .cloned()
            .unwrap_or_else(|| json!({}));

        let response = match method {
            "initialize" => {
                log("info", "Plugin initialized");
                ok(&id, json!({
                    "protocolVersion": PROTOCOL_VERSION,
                    "capabilities": { "tools": {} },
                    "serverInfo": { "name": NAME, "version": VERSION }
                }))
            }
            "ping" | "shutdown" => ok(&id, json!({})),
            "health/check" => ok(&id, json!({ "ok": true })),
            "tools/list" => ok(&id, json!({ "tools": tool_definitions() })),
            "tools/call" => handle_tool_call(&id, &params),
            _ => err(&id, -32601, &format!("Method not found: {method}")),
        };

        let _ = writeln!(stdout, "{}", response);
        let _ = stdout.flush();

        if method == "shutdown" {
            std::process::exit(0);
        }
    }
}

// ---------------------------------------------------------------------------
// Tool definitions
// ---------------------------------------------------------------------------

fn tool_definitions() -> Value {
    json!([
        {
            "name": "convert_image",
            "description": "Convert a single image between formats (supports JPEG, PNG, WebP, GIF, BMP, TIFF, with ImageMagick fallback for AVIF/HEIC)",
            "inputSchema": {
                "type": "object",
                "required": ["input_path", "output_path"],
                "properties": {
                    "input_path": {
                        "type": "string",
                        "description": "Path to the source image file"
                    },
                    "output_path": {
                        "type": "string",
                        "description": "Path for the converted output (format inferred from extension)"
                    },
                    "format": {
                        "type": "string",
                        "enum": ["jpeg", "png", "webp", "gif", "bmp", "tiff", "avif", "heic"],
                        "description": "Override output format (changes output extension)"
                    },
                    "quality": {
                        "type": "integer",
                        "description": "Output quality 1-100 (default: 90, applies to JPEG)",
                        "minimum": 1,
                        "maximum": 100
                    },
                    "width": {
                        "type": "integer",
                        "description": "Target width in pixels",
                        "minimum": 1
                    },
                    "height": {
                        "type": "integer",
                        "description": "Target height in pixels",
                        "minimum": 1
                    },
                    "resize_mode": {
                        "type": "string",
                        "enum": ["fit", "exact"],
                        "description": "Resize mode: 'fit' maintains aspect ratio, 'exact' stretches (default: fit)"
                    }
                }
            }
        },
        {
            "name": "convert_directory",
            "description": "Batch convert all images in a directory to a target format",
            "inputSchema": {
                "type": "object",
                "required": ["input_dir", "format"],
                "properties": {
                    "input_dir": {
                        "type": "string",
                        "description": "Path to the source directory"
                    },
                    "output_dir": {
                        "type": "string",
                        "description": "Path for converted output (defaults to input_dir + '_converted')"
                    },
                    "format": {
                        "type": "string",
                        "enum": ["jpeg", "png", "webp", "gif", "bmp", "tiff", "avif", "heic"],
                        "description": "Target format for all images"
                    },
                    "quality": {
                        "type": "integer",
                        "description": "Output quality 1-100 (default: 90)",
                        "minimum": 1,
                        "maximum": 100
                    }
                }
            }
        }
    ])
}

// ---------------------------------------------------------------------------
// Tool dispatch
// ---------------------------------------------------------------------------

fn handle_tool_call(id: &Value, params: &Value) -> Value {
    let tool_name = params
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let args = params
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| json!({}));

    match tool_name {
        "convert_image" => call_convert_image(id, &args),
        "convert_directory" => call_convert_directory(id, &args),
        _ => err(id, -32601, &format!("Unknown tool: {tool_name}")),
    }
}

fn call_convert_image(id: &Value, args: &Value) -> Value {
    let Some(input_path) = args.get("input_path").and_then(Value::as_str) else {
        return err(id, -32602, "Missing required parameter: input_path");
    };
    let Some(output_path_raw) = args.get("output_path").and_then(Value::as_str) else {
        return err(id, -32602, "Missing required parameter: output_path");
    };

    // If format is specified, override the output extension
    let output_path = if let Some(fmt) = args.get("format").and_then(Value::as_str) {
        let ext = normalize_format_ext(fmt);
        Path::new(output_path_raw)
            .with_extension(ext)
            .to_string_lossy()
            .into_owned()
    } else {
        output_path_raw.to_string()
    };

    let resize_mode = match args.get("resize_mode").and_then(Value::as_str) {
        Some("exact") => ResizeMode::Exact,
        _ => ResizeMode::Fit,
    };

    let width = args.get("width").and_then(Value::as_u64).map(|v| v as u32);
    let height = args.get("height").and_then(Value::as_u64).map(|v| v as u32);

    let resize = match (width, height) {
        (Some(w), Some(h)) => ResizeOptions::new(w, h, resize_mode).ok(),
        (Some(w), None) => ResizeOptions::new(w, u32::MAX, resize_mode).ok(),
        (None, Some(h)) => ResizeOptions::new(u32::MAX, h, resize_mode).ok(),
        _ => None,
    };

    let options = ConvertOptions {
        overwrite: true,
        quality: args
            .get("quality")
            .and_then(Value::as_u64)
            .map(|v| v as u8)
            .or(Some(90)),
        resize,
        ..ConvertOptions::default()
    };

    log("info", &format!("convert_image: {input_path} -> {output_path}"));

    match convert_image_file(Path::new(input_path), Path::new(&output_path), options) {
        Ok(()) => {
            let text = format!("Converted {input_path} -> {output_path}");
            ok(id, json!({
                "content": [{ "type": "text", "text": text }]
            }))
        }
        Err(e) => err(id, -32000, &format!("Conversion failed: {e:#}")),
    }
}

fn call_convert_directory(id: &Value, args: &Value) -> Value {
    let Some(input_dir) = args.get("input_dir").and_then(Value::as_str) else {
        return err(id, -32602, "Missing required parameter: input_dir");
    };
    let Some(format) = args.get("format").and_then(Value::as_str) else {
        return err(id, -32602, "Missing required parameter: format");
    };

    let ext = normalize_format_ext(format);

    let output_dir = match args.get("output_dir").and_then(Value::as_str) {
        Some(p) => p.to_string(),
        None => format!("{input_dir}_converted"),
    };

    let options = ConvertOptions {
        overwrite: true,
        quality: args
            .get("quality")
            .and_then(Value::as_u64)
            .map(|v| v as u8)
            .or(Some(90)),
        ..ConvertOptions::default()
    };

    log("info", &format!("convert_directory: {input_dir} -> {output_dir} (format: {ext})"));

    match convert_directory(
        Path::new(input_dir),
        Path::new(&output_dir),
        ext,
        options,
        true,
    ) {
        Ok(report) => {
            let text = format!(
                "Batch conversion complete: {} converted, {} skipped, {} failed",
                report.converted, report.skipped, report.failed,
            );
            ok(id, json!({
                "content": [{ "type": "text", "text": text }]
            }))
        }
        Err(e) => err(id, -32000, &format!("Batch conversion failed: {e:#}")),
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn normalize_format_ext(format: &str) -> &str {
    match format {
        "jpeg" => "jpg",
        "tiff" => "tif",
        other => other,
    }
}

// ---------------------------------------------------------------------------
// JSON-RPC helpers
// ---------------------------------------------------------------------------

fn ok(id: &Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

fn err(id: &Value, code: i32, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

fn log(level: &str, msg: &str) {
    let entry = json!({
        "ts": chrono_now(),
        "level": level,
        "plugin": NAME,
        "msg": msg
    });
    eprintln!("{entry}");
}

fn chrono_now() -> String {
    use std::time::SystemTime;
    let d = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = d.as_secs();
    format!("{secs}")
}
