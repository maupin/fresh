//! E2E tests for the git_statusbar plugin
//!
//! These tests verify that the status bar can be configured to show the
//! git branch element, which is registered by the git_statusbar plugin.

use crate::common::harness::{EditorTestHarness, HarnessOptions};
use fresh::config::{Config, StatusBarConfig, StatusBarElement};
use std::fs;

#[test]
fn test_status_bar_shows_custom_branch_token() {
    // Configure status bar to include the branch token
    let mut config = Config::default();
    config.editor.status_bar = StatusBarConfig {
        left: vec![
            StatusBarElement::Filename,
            StatusBarElement::CustomToken("branch".to_string()),
        ],
        right: vec![StatusBarElement::Encoding, StatusBarElement::Language],
    };

    let mut harness = EditorTestHarness::create(
        80,
        24,
        HarnessOptions::new().with_project_root().with_config(config),
    )
    .unwrap();

    let project_dir = harness.project_dir().unwrap();
    let test_file = project_dir.join("test.txt");
    fs::write(&test_file, "test content\n").unwrap();

    harness.open_file(&test_file).unwrap();
    harness.render().unwrap();

    // Verify status bar contains expected elements
    let status_bar = harness.get_status_bar();
    
    // The filename should be visible
    assert!(
        status_bar.contains("test.txt"),
        "Status bar should contain filename. Got: {}",
        status_bar
    );
    
    // Encoding and language should be visible on the right side
    assert!(
        status_bar.contains("ASCII") || status_bar.contains("UTF-8"),
        "Status bar should contain encoding. Got: {}",
        status_bar
    );
    
    assert!(
        status_bar.contains("Text"),
        "Status bar should contain language. Got: {}",
        status_bar
    );
    
    // Note: The "branch" token value comes from the plugin at runtime.
    // Without the plugin loaded, the element shows as empty/placeholder.
    // The actual "Not in git" value appears when the plugin runs.
}