#[test]
fn all_wgsl_shaders_parse() {
    for entry in std::fs::read_dir("assets/shaders").unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) != Some("wgsl") {
            continue;
        }
        let source = std::fs::read_to_string(&path).unwrap();
        wgpu::naga::front::wgsl::parse_str(&source)
            .unwrap_or_else(|e| panic!("{} failed to parse: {e}", path.display()));
    }
}

#[test]
fn showcase_scene_parses_and_validates() {
    let text = std::fs::read_to_string("assets/scenes/showcase.json").unwrap();
    let scene = gxengine::game::scene::Scene::from_json(&text).expect("showcase.json must parse");
    assert!(scene.tornado.is_some(), "showcase declares a tornado");
    assert!(scene.water.is_some(), "showcase declares water");
    assert_eq!(scene.structures.len(), 2, "two destructible structures");
    let warnings = scene.validation_warnings();
    assert!(
        warnings.is_empty(),
        "showcase should be warning-free: {warnings:#?}"
    );
}
