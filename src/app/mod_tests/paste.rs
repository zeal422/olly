use super::helpers::*;
use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

#[test]
fn resolve_mouse_event_right_click_returns_paste() {
    let event = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Right),
        column: 10,
        row: 5,
        modifiers: KeyModifiers::NONE,
    };
    assert_eq!(
        crate::app::input::resolve_mouse_event(event),
        crate::app::input::InputAction::Paste
    );
}

#[test]
fn resolve_mouse_event_left_click_ignored() {
    let event = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 10,
        row: 5,
        modifiers: KeyModifiers::NONE,
    };
    assert_eq!(
        crate::app::input::resolve_mouse_event(event),
        crate::app::input::InputAction::Noop
    );
}

#[test]
fn resolve_mouse_event_middle_click_ignored() {
    let event = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Middle),
        column: 10,
        row: 5,
        modifiers: KeyModifiers::NONE,
    };
    assert_eq!(
        crate::app::input::resolve_mouse_event(event),
        crate::app::input::InputAction::Noop
    );
}

#[test]
fn insert_input_string_at_cursor() {
    let mut app = App::new(
        String::from("http://localhost:11434"),
        String::from("llama3.2:latest"),
    );

    app.set_input_text(String::from("Hello"));
    app.input_cursor_char_index = 5; // At end

    app.insert_input_string(String::from(" world"));

    assert_eq!(app.input, "Hello world");
    assert_eq!(app.input_cursor_char_index, 11); // 5 + 6 chars
}

#[test]
fn insert_input_string_at_middle() {
    let mut app = App::new(
        String::from("http://localhost:11434"),
        String::from("llama3.2:latest"),
    );

    app.set_input_text(String::from("Hello world"));
    app.input_cursor_char_index = 5; // After "Hello"

    app.insert_input_string(String::from(" beautiful"));

    assert_eq!(app.input, "Hello beautiful world");
    assert_eq!(app.input_cursor_char_index, 15); // 5 + 10 chars
}

#[test]
fn insert_input_string_empty_does_nothing() {
    let mut app = App::new(
        String::from("http://localhost:11434"),
        String::from("llama3.2:latest"),
    );

    app.set_input_text(String::from("Hello"));
    app.input_cursor_char_index = 2;

    app.insert_input_string(String::from(""));

    assert_eq!(app.input, "Hello");
    assert_eq!(app.input_cursor_char_index, 2);
}

#[test]
fn insert_input_string_multiline() {
    let mut app = App::new(
        String::from("http://localhost:11434"),
        String::from("llama3.2:latest"),
    );

    app.set_input_text(String::from("Line1"));
    app.input_cursor_char_index = 5; // At end

    app.insert_input_string(String::from("\nLine2"));

    assert_eq!(app.input, "Line1\nLine2");
    assert_eq!(app.input_cursor_char_index, 11); // 5 + 6 chars
}

#[test]
fn insert_input_string_at_start() {
    let mut app = App::new(
        String::from("http://localhost:11434"),
        String::from("llama3.2:latest"),
    );

    app.set_input_text(String::from("world"));
    app.input_cursor_char_index = 0; // At start

    app.insert_input_string(String::from("Hello "));

    assert_eq!(app.input, "Hello world");
    assert_eq!(app.input_cursor_char_index, 6);
}

#[test]
fn resolve_mouse_event_right_click_returns_paste_in_chat_focus() {
    // This test verifies that right-click mouse events resolve to InputAction::Paste
    // regardless of app focus state. The focus-dependent behavior is enforced
    // in the paste handler, not in the event resolver.
    let mut app = App::new(
        String::from("http://localhost:11434"),
        String::from("llama3.2:latest"),
    );
    app.focus = crate::app::FocusPane::Chat;
    app.set_input_text(String::from("Hello"));
    app.input_cursor_char_index = 5;

    let event = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Right),
        column: 10,
        row: 5,
        modifiers: KeyModifiers::NONE,
    };
    let action = crate::app::input::resolve_mouse_event(event);

    assert_eq!(action, crate::app::input::InputAction::Paste);
}

#[test]
fn resolve_mouse_event_right_click_returns_paste_in_workspace_with_command() {
    // Note: App state is unused here since resolve_mouse_event is stateless.
    // The focus-dependent behavior is enforced in the paste handler.
    let mut app = App::new(
        String::from("http://localhost:11434"),
        String::from("llama3.2:latest"),
    );
    app.focus = crate::app::FocusPane::Workspaces;
    app.set_input_text(String::from("/workspace "));

    let event = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Right),
        column: 10,
        row: 5,
        modifiers: KeyModifiers::NONE,
    };
    let action = crate::app::input::resolve_mouse_event(event);

    assert_eq!(action, crate::app::input::InputAction::Paste);
}

#[test]
fn resolve_mouse_event_returns_paste_regardless_of_focus() {
    // Note: resolve_mouse_event is stateless and always returns InputAction::Paste
    // for right-click. Focus-dependent ignore logic lives in the paste handler.
    let event = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Right),
        column: 10,
        row: 5,
        modifiers: KeyModifiers::NONE,
    };
    let action = crate::app::input::resolve_mouse_event(event);

    assert_eq!(action, crate::app::input::InputAction::Paste);
}
