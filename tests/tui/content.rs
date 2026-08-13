use super::support::*;

#[test]
fn content_detail_preserves_exact_provenance_values() {
    let (_root, mut app) = fixture_app();
    app.open(Screen::ContentDetail);
    let output = buffer_text(&draw(&app, 120, 40).buffer);

    for value in [
        "en-tatoeba-331259",
        "Tatoeba CC0 contributors",
        "tatoeba:331259",
        "https://tatoeba.org/en/sentences/show/331259",
        "CC0-1.0",
        "https://creativecommons.org/publicdomain/zero/1.0/",
        "2026-08-07",
        "modified: no",
    ] {
        assert!(output.contains(value), "missing {value:?}: {output}");
    }
}

#[test]
fn content_detail_pages_through_pack_and_every_unique_item_provenance() {
    let (_root, mut app) = fixture_app();
    app.warnings.clear();
    app.open(Screen::Content);
    let index = app
        .content_packs()
        .iter()
        .position(|pack| pack.id == "en-sentences")
        .unwrap();
    for _ in 0..index {
        app.handle_event(key(Key::Tab), Instant::now()).unwrap();
    }
    app.handle_event(key(Key::Enter), Instant::now()).unwrap();

    let first = buffer_text(&draw(&app, 120, 40).buffer);
    assert!(first.contains("Provenance 1/121"), "{first}");
    assert!(first.contains("tatoeba:331259"), "{first}");
    app.handle_event(key(Key::Down), Instant::now()).unwrap();
    let second = buffer_text(&draw(&app, 120, 40).buffer);
    assert!(second.contains("Provenance 2/121"), "{second}");
    assert!(second.contains("tatoeba:337215"), "{second}");
    app.handle_event(key(Key::Up), Instant::now()).unwrap();
    app.handle_event(key(Key::Up), Instant::now()).unwrap();
    let pack = buffer_text(&draw(&app, 120, 40).buffer);
    assert!(pack.contains("Provenance 121/121"), "{pack}");
    assert!(pack.contains("scope: pack"), "{pack}");
    assert!(
        pack.contains(
            "tatoeba-eng_cc0-6ab169264a28008c25bf63042bf7535fc63137c9d7e09b7b8bd7812d10117d1b"
        ),
        "{pack}"
    );
}

#[test]
fn content_detail_keeps_provenance_license_and_status_visible_with_a_warning() {
    let (_root, mut app) = fixture_app();
    app.open(Screen::Content);
    let index = app
        .content_packs()
        .iter()
        .position(|pack| pack.id == "en-sentences")
        .unwrap();
    for _ in 0..index {
        app.handle_event(key(Key::Tab), Instant::now()).unwrap();
    }
    app.handle_event(key(Key::Enter), Instant::now()).unwrap();
    app.handle_event(key(Key::Up), Instant::now()).unwrap();

    let output = buffer_text(&draw(&app, 80, 24).buffer);
    for value in [
        "scope: pack",
        "tatoeba-eng_cc0-",
        "typerlude licenses",
        "Built-in packs cannot be disabled",
        "review warning",
    ] {
        assert!(output.contains(value), "missing {value:?}: {output}");
    }
}

#[test]
fn content_packs_group_provenance_and_disable_only_users_after_confirmation() {
    let root = TestDir::new();
    let paths = AppPaths::from_override(root.path().join("home"));
    fs::create_dir_all(&paths.content).unwrap();
    let source = paths.content.join("user-pack.toml");
    fs::write(&source, user_pack("user-pack")).unwrap();
    let loaded = ContentCatalog::load(&paths.content).unwrap();
    assert!(loaded.warnings.is_empty());
    let mut app = App::new(
        Settings::default(),
        paths.clone(),
        loaded.catalog,
        ThemeCatalog::load_builtins().unwrap(),
        Vec::new(),
        Vec::new(),
    );
    let summaries = app.content_packs();
    let user = summaries
        .iter()
        .find(|summary| summary.id == "user-pack")
        .unwrap();
    assert_eq!(user.language, Language::En);
    assert_eq!(user.items, 1);
    assert_eq!(user.licenses, vec!["CC-BY-4.0", "CC0-1.0"]);
    assert!(user.enabled);
    assert!(!user.built_in);
    assert!(summaries.iter().any(|summary| summary.built_in));
    fs::write(paths.content.join("broken.toml"), b"schema_version = [").unwrap();

    app.open(Screen::Content);
    let user_index = app
        .content_packs()
        .iter()
        .position(|summary| summary.id == "user-pack")
        .unwrap();
    for _ in 0..user_index {
        app.handle_event(key(Key::Tab), Instant::now()).unwrap();
    }
    app.handle_event(key(Key::Enter), Instant::now()).unwrap();
    assert_eq!(app.screen(), Screen::ContentDetail);
    assert_eq!(app.selected_content_pack(), Some("user-pack"));
    let detail = buffer_text(&draw(&app, 120, 40).buffer);
    for value in [
        "Test author",
        "test-source",
        "https://example.com/source",
        "CC0-1.0",
        "https://creativecommons.org/publicdomain/zero/1.0/",
        "2026-08-07",
        "typerlude content add PACK.toml",
        "typerlude content validate PACK.toml",
        "d: Disable",
    ] {
        assert!(detail.contains(value), "missing {value:?}: {detail}");
    }

    app.handle_event(key(Key::Char('d')), Instant::now())
        .unwrap();
    assert!(source.exists());
    assert!(buffer_text(&draw(&app, 120, 40).buffer).contains("Press d again"));
    app.handle_event(key(Key::Char('d')), Instant::now())
        .unwrap();
    assert!(!source.exists());
    assert!(paths.content.join("disabled/user-pack.toml").exists());
    assert!(!app.content.contains_pack("user-pack"));
    assert!(
        app.warnings
            .iter()
            .any(|warning| warning.contains("pack=broken")),
        "{:?}",
        app.warnings
    );
    let disabled = app
        .content_packs()
        .iter()
        .find(|summary| summary.id == "user-pack")
        .unwrap();
    assert!(!disabled.enabled);
    assert!(!disabled.built_in);

    app.open(Screen::Content);
    let disabled_index = app
        .content_packs()
        .iter()
        .position(|summary| summary.id == "user-pack")
        .unwrap();
    for _ in 0..disabled_index {
        app.handle_event(key(Key::Tab), Instant::now()).unwrap();
    }
    app.handle_event(key(Key::Enter), Instant::now()).unwrap();
    let disabled_detail = buffer_text(&draw(&app, 120, 40).buffer);
    for value in [
        "Test author",
        "test-source",
        "CC0-1.0",
        "User pack is disabled",
    ] {
        assert!(
            disabled_detail.contains(value),
            "missing {value:?}: {disabled_detail}"
        );
    }

    app.open(Screen::Content);
    let built_in_index = app
        .content_packs()
        .iter()
        .position(|summary| summary.built_in)
        .unwrap();
    for _ in 0..built_in_index {
        app.handle_event(key(Key::Tab), Instant::now()).unwrap();
    }
    app.handle_event(key(Key::Enter), Instant::now()).unwrap();
    app.handle_event(key(Key::Char('d')), Instant::now())
        .unwrap();
    assert!(
        app.warnings
            .last()
            .is_some_and(|warning| warning.contains("built-in")),
        "{:?}",
        app.warnings
    );
}

#[cfg(unix)]
#[test]
fn content_pack_listing_does_not_follow_a_disabled_directory_symlink() {
    use std::os::unix::fs::symlink;

    let root = TestDir::new();
    let paths = AppPaths::from_override(root.path().join("home"));
    let outside = root.path().join("outside");
    fs::create_dir_all(&outside).unwrap();
    fs::create_dir_all(&paths.content).unwrap();
    fs::write(outside.join("escaped-pack.toml"), user_pack("escaped-pack")).unwrap();
    symlink(&outside, paths.content.join("disabled")).unwrap();
    let app = App::new(
        Settings::default(),
        paths,
        ContentCatalog::load_builtins().unwrap(),
        ThemeCatalog::load_builtins().unwrap(),
        Vec::new(),
        Vec::new(),
    );

    assert!(
        app.content_packs()
            .iter()
            .all(|pack| pack.id != "escaped-pack")
    );
}
