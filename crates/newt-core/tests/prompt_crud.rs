//! Tests for prompt::create / update / delete + id validation.

use newt_core::{
    paths::Paths,
    prompt::{self, PromptInput, validate_id},
};

fn tempdir() -> std::path::PathBuf {
    let base = std::env::temp_dir().join(format!(
        "newt-prompt-crud-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&base).unwrap();
    base
}

fn input(name: &str, instructions: &str) -> PromptInput {
    PromptInput {
        name: name.to_string(),
        description: format!("test prompt for {name}"),
        emoji: Some("🧪".to_string()),
        model: None,
        instructions: instructions.to_string(),
    }
}

#[test]
fn validate_id_accepts_well_formed_kebab_case() {
    for id in ["a1", "improve-writing", "my-custom-prompt-7", "t9"] {
        validate_id(id).unwrap_or_else(|e| panic!("`{id}` should be valid: {e}"));
    }
}

#[test]
fn validate_id_rejects_malformed() {
    for bad in [
        "x",         // too short
        "-leading",  // leading hyphen
        "trailing-", // trailing hyphen
        "double--hyphen",
        "Caps", // uppercase
        "with space",
        "with/slash",
        "with.dot",
        "../escape",
        "",
    ] {
        assert!(validate_id(bad).is_err(), "`{bad}` should be rejected");
    }
}

#[test]
fn create_writes_a_new_prompt_and_round_trips_via_load() {
    let paths = Paths::with_home(tempdir());
    let inp = input("Test prompt", "Do a thing.");
    prompt::create(&paths, "test-prompt", &inp).expect("create");

    let loaded = prompt::load(&paths, "test-prompt").expect("load");
    assert_eq!(loaded.id, "test-prompt");
    assert_eq!(loaded.name, "Test prompt");
    assert_eq!(loaded.emoji.as_deref(), Some("🧪"));
    assert_eq!(loaded.instructions, "Do a thing.");
}

#[test]
fn create_refuses_to_overwrite_existing_disk_file() {
    let paths = Paths::with_home(tempdir());
    prompt::create(&paths, "alpha", &input("Alpha", "x")).unwrap();
    let err = prompt::create(&paths, "alpha", &input("Alpha2", "y")).unwrap_err();
    assert!(err.to_string().contains("already exists"));
}

#[test]
fn create_refuses_to_shadow_a_bundled_default() {
    let paths = Paths::with_home(tempdir());
    let err = prompt::create(&paths, "improve-writing", &input("Mine", "x")).unwrap_err();
    assert!(err.to_string().contains("bundled default"));
}

#[test]
fn update_modifies_existing_disk_prompt() {
    let paths = Paths::with_home(tempdir());
    prompt::create(&paths, "thing", &input("Thing", "v1")).unwrap();

    let mut updated = input("Thing renamed", "v2 instructions");
    updated.description = "new description".into();
    prompt::update(&paths, "thing", &updated).unwrap();

    let loaded = prompt::load(&paths, "thing").unwrap();
    assert_eq!(loaded.name, "Thing renamed");
    assert_eq!(loaded.description, "new description");
    assert_eq!(loaded.instructions, "v2 instructions");
}

#[test]
fn update_persists_a_user_override_for_a_bundled_default() {
    let paths = Paths::with_home(tempdir());
    let custom = input("My improved writing", "Custom instructions go here.");
    prompt::update(&paths, "improve-writing", &custom).unwrap();

    let loaded = prompt::load(&paths, "improve-writing").unwrap();
    assert_eq!(loaded.name, "My improved writing");
    assert_eq!(loaded.instructions, "Custom instructions go here.");
}

#[test]
fn update_errors_when_neither_disk_nor_bundled() {
    let paths = Paths::with_home(tempdir());
    let err = prompt::update(&paths, "nonexistent", &input("X", "y")).unwrap_err();
    assert!(err.to_string().contains("no prompt"));
}

#[test]
fn delete_removes_disk_file_and_returns_true() {
    let paths = Paths::with_home(tempdir());
    prompt::create(&paths, "doomed", &input("Doomed", "x")).unwrap();
    assert!(prompt::delete(&paths, "doomed").unwrap());
    assert!(!prompt::delete(&paths, "doomed").unwrap()); // already gone
}

#[test]
fn delete_of_bundled_default_override_makes_default_visible_again() {
    let paths = Paths::with_home(tempdir());
    let custom = input("Override", "Custom body.");
    prompt::update(&paths, "improve-writing", &custom).unwrap();
    assert_eq!(
        prompt::load(&paths, "improve-writing").unwrap().name,
        "Override"
    );

    assert!(prompt::delete(&paths, "improve-writing").unwrap());
    // Now the bundled default reappears.
    assert_eq!(
        prompt::load(&paths, "improve-writing").unwrap().name,
        "Improve writing"
    );
}

#[test]
fn create_validates_required_fields() {
    let paths = Paths::with_home(tempdir());
    let mut bad_name = input("Test", "x");
    bad_name.name = "   ".into();
    assert!(prompt::create(&paths, "x1", &bad_name).is_err());

    let mut bad_body = input("Test", "x");
    bad_body.instructions = "".into();
    assert!(prompt::create(&paths, "x2", &bad_body).is_err());
}
