use crate::app::App;

#[test]
fn push_assistant_resets_scroll_to_bottom() {
    let mut app = App::new(
        String::from("http://localhost:11434"),
        String::from("llama3.2:latest"),
    );
    app.chat_scroll = 500;

    app.push_assistant(String::from("test response"), None);

    assert_eq!(app.chat_scroll, 0);
}

#[test]
fn push_user_resets_scroll_to_bottom() {
    let mut app = App::new(
        String::from("http://localhost:11434"),
        String::from("llama3.2:latest"),
    );
    app.chat_scroll = 500;

    app.push_user(String::from("test message"), 0);

    assert_eq!(app.chat_scroll, 0);
}

#[test]
fn scroll_up_on_empty_chat_does_not_panic() {
    let mut app = App::new(
        String::from("http://localhost:11434"),
        String::from("llama3.2:latest"),
    );
    app.update_chat_max_scroll_hint(0);
    app.chat_scroll = 0;

    app.scroll_up(30);

    assert_eq!(app.chat_scroll, 0);
}

#[test]
fn scroll_down_on_empty_chat_caps_at_zero() {
    let mut app = App::new(
        String::from("http://localhost:11434"),
        String::from("llama3.2:latest"),
    );
    app.update_chat_max_scroll_hint(0);
    app.chat_scroll = 0;

    app.scroll_down(30);

    assert_eq!(app.chat_scroll, 0);
}
