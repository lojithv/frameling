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
