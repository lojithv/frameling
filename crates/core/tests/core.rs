use frameling_core::*;

#[test]
fn evaluation_is_deterministic() {
    let p = demo::demo_project();
    let lib = Library::builtin();
    for f in [0, 37, 119, 125, 250, 329] {
        assert_eq!(evaluate(&p, lib, f), evaluate(&p, lib, f), "frame {f}");
    }
}

#[test]
fn project_json_round_trips() {
    let p = demo::demo_project();
    let back = Project::from_json(&p.to_json()).unwrap();
    assert_eq!(p, back);
}

#[test]
fn timeline_locates_frames() {
    let p = demo::demo_project();
    assert_eq!(p.total_frames(), 330);
    assert_eq!(p.locate(0), Some((0, 0)));
    assert_eq!(p.locate(120), Some((1, 0)));
    assert_eq!(p.locate(329), Some((2, 89)));
    assert_eq!(p.locate(330), None);
}

#[test]
fn undo_redo_and_merge() {
    let mut ed = Editor::new(demo::demo_project());
    let mut item = ed.project.scenes[0].items[0].clone();
    for x in [10.0, 20.0, 30.0] {
        item.x = x;
        ed.apply(&Command::UpdateItem { scene: "scene-1".into(), item: item.clone() }, Some("drag")).unwrap();
    }
    assert_eq!(ed.project.scenes[0].items[0].x, 30.0);
    assert!(ed.undo()); // one undo step for the whole drag
    assert_eq!(ed.project.scenes[0].items[0].x, 900.0);
    assert!(ed.redo());
    assert_eq!(ed.project.scenes[0].items[0].x, 30.0);
}

#[test]
fn bad_commands_leave_project_untouched() {
    let mut ed = Editor::new(demo::demo_project());
    let before = ed.project.clone();
    let err = ed.apply(&Command::RemoveItem { scene: "scene-1".into(), id: "nope".into() }, None);
    assert!(err.is_err());
    assert_eq!(ed.project, before);
    assert!(!ed.can_undo());
}

#[test]
fn commands_serialise_as_tagged_json() {
    let c: Command = serde_json::from_str(r#"{"op":"moveScene","id":"scene-2","to":0}"#).unwrap();
    let mut ed = Editor::new(demo::demo_project());
    ed.apply(&c, None).unwrap();
    assert_eq!(ed.project.scenes[0].id, "scene-2");
}

#[test]
fn core_pack_loads_and_custom_pack_overrides() {
    let lib = Library::builtin();
    assert!(lib.props.len() >= 20 && lib.backgrounds.len() >= 8 && lib.actions.contains_key("idle"));
    let mut l = lib.clone();
    l.add_pack_json(r##"{"id":"mine","name":"Mine","props":[{"id":"plant","name":"Cactus","colors":["#00aa00"],"shapes":[{"t":"rect","x":-10,"y":-80,"w":20,"h":80,"fill":"$0"}]}]}"##).unwrap();
    assert_eq!(l.props["plant"].name, "Cactus");
}

#[test]
fn svg_path_parser_handles_relative_and_smooth() {
    let p = Path::parse_svg("M10 10 l 10 0 q 5 5 10 0 t 10 0 C 1 2 3 4 5 6 S 7 8 9 10 z").unwrap();
    assert_eq!(p.0.len(), 7);
}

#[test]
fn add_line_places_lines_back_to_back_and_extends_scene() {
    let mut ed = Editor::new(demo::demo_project());
    let media = |id: &str, frames: u32| Media { id: id.into(), name: id.into(), mime: "audio/wav".into(), duration_frames: frames, envelope: vec![0.5; frames as usize], ..Default::default() };
    ed.apply(&Command::AddLine { scene: "scene-1".into(), clip_id: "l1".into(), media: media("m1", 60), speaker: Some("maya".into()), start: None }, None).unwrap();
    ed.apply(&Command::AddLine { scene: "scene-1".into(), clip_id: "l2".into(), media: media("m2", 90), speaker: Some("leo".into()), start: None }, None).unwrap();
    let s = &ed.project.scenes[0];
    assert_eq!(s.audio[0].start, 10); // fps/3
    assert_eq!(s.audio[1].start, 10 + 60 + 6); // after line 1 + fps/5
    assert_eq!(s.duration, 10 + 60 + 6 + 90 + 15); // grew to fit + half a second
    assert!(ed.apply(&Command::AddLine { scene: "scene-1".into(), clip_id: "l3".into(), media: media("m3", 5), speaker: Some("nobody".into()), start: None }, None).is_err());
}

#[test]
fn wav_envelope_matches_length() {
    // 1 second of 16-bit mono 8 kHz: half silence, half full-scale square wave.
    let rate = 8000u32;
    let mut data = Vec::new();
    for i in 0..rate {
        let v: i16 = if i < rate / 2 { 0 } else if i % 2 == 0 { 20000 } else { -20000 };
        data.extend_from_slice(&v.to_le_bytes());
    }
    let mut wav = Vec::new();
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + data.len() as u32).to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&rate.to_le_bytes());
    wav.extend_from_slice(&(rate * 2).to_le_bytes());
    wav.extend_from_slice(&2u16.to_le_bytes());
    wav.extend_from_slice(&16u16.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&(data.len() as u32).to_le_bytes());
    wav.extend_from_slice(&data);
    let (frames, env) = audio::analyse_wav(&wav, 30).unwrap();
    assert_eq!(frames, 30);
    assert!(env[2] < 0.01 && env[25] > 0.99);
}
