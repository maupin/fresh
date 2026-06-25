use crate::common::harness::{EditorTestHarness, HarnessOptions};
use crossterm::event::{KeyCode, KeyModifiers};
use fresh::config::{Config, IndentationGuideMode};
use tempfile::TempDir;

#[test]
fn indentation_guide_render_configured_glyph_in_editor_flow() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("guides.rs");
    std::fs::write(
        &file_path,
        "fn main() {\n    let child = 1;\n        let grand = child + 1;\n}\n",
    )
    .unwrap();

    let mut config = Config::default();
    config.editor.indentation_guide = IndentationGuideMode::All;
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

#[test]
fn indentation_guide_all_mode_continues_through_blank_line_in_editor_flow() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("guides_blank.rs");
    // The middle line is whitespace-only (four spaces) inside the indented
    // block, so its column-0 guide cell exists and must be drawn.
    std::fs::write(&file_path, "fn main()\n    above\n    \n    below\n").unwrap();

    let mut config = Config::default();
    config.editor.indentation_guide = IndentationGuideMode::All;

    let mut harness =
        EditorTestHarness::create(80, 24, HarnessOptions::new().with_config(config)).unwrap();
    harness.open_file(&file_path).unwrap();
    harness.render().unwrap();

    let screen = harness.screen_to_string();
    let lines: Vec<&str> = screen.lines().collect();
    let above_row = lines
        .iter()
        .position(|line| line.contains("▏   above"))
        .unwrap_or_else(|| panic!("expected a guided 'above' row\n{screen}"));

    // The blank row sits directly below `above` and must carry the guide too,
    // rather than leaving a one-row gap in the vertical line.
    let blank_row = lines[above_row + 1];
    assert!(
        blank_row.contains('▏'),
        "indentation guide should continue through the blank line\nblank row: {blank_row:?}\n{screen}"
    );
    assert!(
        screen.contains("▏   below"),
        "indentation guide should resume on the line after the blank\n{screen}"
    );
}
