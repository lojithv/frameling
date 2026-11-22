use std::io::Write;
use std::process::{Command, Stdio};

#[test]
fn mcp_session_lists_tools_and_edits_the_project() {
    let dir = std::env::temp_dir().join(format!("fl-mcp-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("p.frameling");
    let mut child = Command::new(env!("CARGO_BIN_EXE_frameling"))
        .arg("mcp")
        .arg(&file)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let msgs = [
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18"}}"#,
        r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#,
        r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"apply_commands","arguments":{"commands":[{"op":"addItem","scene":"s1","item":{"id":"tree","x":300,"y":640,"kind":{"type":"prop","prop":"tree"}}}]}}}"#,
        r#"{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"render_frame","arguments":{"scene":"s1","seconds":0.5,"width":320}}}"#,
        r#"{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"nope","arguments":{}}}"#,
    ];
    child.stdin.as_mut().unwrap().write_all((msgs.join("\n") + "\n").as_bytes()).unwrap();
    let out = child.wait_with_output().unwrap();
    let lines: Vec<serde_json::Value> = String::from_utf8(out.stdout).unwrap().lines().map(|l| serde_json::from_str(l).unwrap()).collect();
    assert_eq!(lines.len(), 5, "one reply per request, none for the notification");
    assert!(lines[1]["result"]["tools"].as_array().unwrap().len() >= 10);
    assert_eq!(lines[2]["result"]["content"][0]["text"], "Applied 1 command(s).");
    assert_eq!(lines[3]["result"]["content"][0]["mimeType"], "image/png");
    assert_eq!(lines[4]["result"]["isError"], true);
    let saved = std::fs::read_to_string(&file).unwrap();
    assert!(saved.contains("\"tree\""), "project saved after each change");
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn mcp_keyframe_tools_animate_and_save() {
    let dir = std::env::temp_dir().join(format!("fl-mcp-keys-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("p.frameling");
    let mut child = Command::new(env!("CARGO_BIN_EXE_frameling"))
        .arg("mcp")
        .arg(&file)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let call = |id: u32, name: &str, args: serde_json::Value| {
        serde_json::json!({ "jsonrpc": "2.0", "id": id, "method": "tools/call", "params": { "name": name, "arguments": args } }).to_string()
    };
    use serde_json::json;
    let msgs = [
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18"}}"#.to_string(),
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#.to_string(),
        call(3, "new_project", json!({ "template": "demo" })),
        call(4, "define_outfit", json!({ "scene": "scene-1", "item": "leo", "name": "suit", "outfit": { "top": "#222222", "accessory": "tie" } })),
        call(5, "set_keys", json!({ "scene": "scene-1", "item": "leo", "path": "outfit", "keys": [{ "seconds": 0, "value": "default" }, { "seconds": 1, "value": "suit" }] })),
        call(6, "set_keys", json!({ "scene": "scene-1", "item": "leo", "path": "transform.x", "keys": [{ "frame": 0, "value": 400, "interp": "easeInOut" }, { "seconds": 2, "value": 900 }] })),
        call(7, "set_keys", json!({ "scene": "scene-1", "item": "leo", "path": "bone.le", "keys": [{ "frame": 0, "value": 0, "interp": { "bezier": [0.3, 0, 0.2, 1.4] } }, { "frame": 20, "value": 120 }] })),
        call(8, "set_keys", json!({ "scene": "scene-1", "path": "camera.zoom", "keys": [{ "frame": 0, "value": 1 }, { "frame": 60, "value": 1.5, "interp": "bounce" }] })),
        call(9, "set_keys", json!({ "scene": "scene-1", "item": "leo", "path": "bone.tail", "keys": [{ "frame": 0, "value": 1 }] })),
        call(10, "set_interpolation", json!({ "scene": "scene-1", "item": "leo", "path": "transform.x", "interp": "elastic", "seconds": [0] })),
        call(11, "list_tracks", json!({ "scene": "scene-1", "item": "leo" })),
        call(12, "render_frame", json!({ "scene": "scene-1", "seconds": 1.5, "width": 320 })),
        call(13, "delete_keys", json!({ "scene": "scene-1", "item": "leo", "path": "bone.forearm_l.rot" })),
        call(14, "get_project", json!({})),
    ];
    child.stdin.as_mut().unwrap().write_all((msgs.join("\n") + "\n").as_bytes()).unwrap();
    let out = child.wait_with_output().unwrap();
    let lines: Vec<serde_json::Value> = String::from_utf8(out.stdout).unwrap().lines().map(|l| serde_json::from_str(l).unwrap()).collect();
    assert_eq!(lines.len(), msgs.len());
    let names: Vec<&str> = lines[1]["result"]["tools"].as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap()).collect();
    for t in ["set_keys", "delete_keys", "list_tracks", "set_interpolation", "define_outfit"] {
        assert!(names.contains(&t), "{t} listed");
    }
    let text = |i: usize| lines[i]["result"]["content"][0]["text"].as_str().unwrap_or("").to_string();
    for i in [3, 4, 5, 6, 7, 9, 10, 12] {
        assert!(lines[i]["result"]["isError"].is_null(), "call {} failed: {}", i + 1, text(i));
    }
    assert_eq!(lines[8]["result"]["isError"], true, "unknown path is an error");
    assert!(text(8).contains("valid paths"));
    let listed = text(10);
    assert!(listed.contains("frame 30 (1.00s) = \"suit\""), "{listed}");
    assert!(listed.contains("\"elastic\""), "{listed}");
    assert!(listed.contains("bone.forearm_l.rot"), "alias stored canonically: {listed}");
    assert_eq!(lines[11]["result"]["content"][0]["mimeType"], "image/png");
    assert!(text(13).contains("keyed: outfit(2), transform.x(2)"), "{}", text(13));
    assert!(text(13).contains("camera keyed: camera.zoom(2)"));
    let saved = std::fs::read_to_string(&file).unwrap();
    assert!(saved.contains("\"tracks\"") && saved.contains("\"suit\"") && saved.contains("\"version\": 2"));
    std::fs::remove_dir_all(dir).unwrap();
}
