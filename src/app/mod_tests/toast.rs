use super::helpers::*;

#[test]
fn toast_creation_preserves_level_and_message() {
    let mut app = App::new(
        String::from("http://localhost:11434"),
        String::from("llama3.2:latest"),
    );

    app.set_toast(crate::app::ToastLevel::Info, "Test message");
    assert!(app.toast.is_some());
    let toast = app.toast.as_ref().unwrap();
    assert_eq!(toast.level, crate::app::ToastLevel::Info);
    assert_eq!(toast.message, "Test message");
}

#[test]
fn toast_dismissal_via_dismiss_overlay() {
    let mut app = App::new(
        String::from("http://localhost:11434"),
        String::from("llama3.2:latest"),
    );

    app.set_toast(crate::app::ToastLevel::Success, "Test toast");
    assert!(app.toast.is_some());

    app.dismiss_overlay();

    assert!(app.toast.is_none());
}

#[test]
fn toast_expires_after_tick() {
    let mut app = App::new(
        String::from("http://localhost:11434"),
        String::from("llama3.2:latest"),
    );

    app.set_toast(crate::app::ToastLevel::Error, "Expiring toast");
    assert!(app.toast.is_some());

    // Manually set expires_at to the past
    if let Some(toast) = &mut app.toast {
        toast.expires_at = std::time::Instant::now() - std::time::Duration::from_secs(1);
    }

    app.tick();

    assert!(app.toast.is_none());
}

#[test]
fn toast_multiple_sets_replace() {
    let mut app = App::new(
        String::from("http://localhost:11434"),
        String::from("llama3.2:latest"),
    );

    app.set_toast(crate::app::ToastLevel::Info, "First toast");
    assert_eq!(app.toast.as_ref().unwrap().message, "First toast");

    app.set_toast(crate::app::ToastLevel::Error, "Second toast");
    assert!(app.toast.is_some());
    assert_eq!(app.toast.as_ref().unwrap().message, "Second toast");
    assert_eq!(
        app.toast.as_ref().unwrap().level,
        crate::app::ToastLevel::Error
    );
}
