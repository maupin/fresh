use crate::common::harness::{EditorTestHarness, HarnessOptions};
use crossterm::event::{KeyCode, KeyModifiers};
use fresh::config::{Config, IndentationGuideMode};
use tempfile::TempDir;

#[test]
fn indentation_guides_render_configured_glyph_in_editor_flow() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("guides.rs");
    std::fs::write(
        &file_path,
        "fn main() {\n    let child = 1;\n        let grand = child + 1;\n}\n",
    )
    .unwrap();

    let mut config = Config::default();
    config.editor.indentation_guides = IndentationGuideMode::All;
    config.editor.indentation_guide_glyph = "┊".to_string();

    let mut harness =
        EditorTestHarness::create(80, 24, HarnessOptions::new().with_config(config)).unwrap();
    harness.open_file(&file_path).unwrap();
    harness.render().unwrap();

    harness.send_key(KeyCode::Down, KeyModifiers::NONE).unwrap();
    harness.render().unwrap();

    let screen = harness.screen_to_string();
    assert!(
        screen.contains("┊   let child = 1;"),
        "configured indentation guide glyph should render on the child line\n{screen}"
    );
    assert!(
        screen.contains("┊   ┊   let grand = child + 1;"),
        "configured indentation guide glyph should render at nested indentation levels\n{screen}"
    );
}
