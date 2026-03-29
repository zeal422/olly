use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use olly::app::input::{
    resolve_key_event, resolve_key_event_with_bindings, InputAction, InputBindings,
};
use olly::config::KeybindingsConfig;

#[test]
fn ctrl_enter_sends_enter_adds_newline() {
    let ctrl_enter = KeyEvent::new(KeyCode::Enter, KeyModifiers::CONTROL);
    let plain_enter = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);

    assert_eq!(resolve_key_event(ctrl_enter), InputAction::Send);
    assert_eq!(resolve_key_event(plain_enter), InputAction::NewLine);
}

#[test]
fn ctrl_q_quits() {
    let ctrl_q = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::CONTROL);
    assert_eq!(resolve_key_event(ctrl_q), InputAction::Quit);
}

#[test]
fn navigation_and_help_keys() {
    let up = KeyEvent::new(KeyCode::Up, KeyModifiers::NONE);
    let page_down = KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE);
    let left = KeyEvent::new(KeyCode::Left, KeyModifiers::NONE);
    let right = KeyEvent::new(KeyCode::Right, KeyModifiers::NONE);
    let help = KeyEvent::new(KeyCode::F(1), KeyModifiers::NONE);
    let esc = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
    let tab = KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE);

    assert_eq!(resolve_key_event(up), InputAction::ScrollUp(1));
    assert_eq!(resolve_key_event(page_down), InputAction::ScrollDown(8));
    assert_eq!(resolve_key_event(left), InputAction::MoveCursorLeft);
    assert_eq!(resolve_key_event(right), InputAction::MoveCursorRight);
    assert_eq!(resolve_key_event(help), InputAction::ToggleHelp);
    assert_eq!(resolve_key_event(esc), InputAction::DismissOverlay);
    assert_eq!(resolve_key_event(tab), InputAction::SwitchFocus);
}

#[test]
fn alt_navigation_keys_keep_standard_scroll_actions() {
    let alt_up = KeyEvent::new(KeyCode::Up, KeyModifiers::ALT);
    let alt_down = KeyEvent::new(KeyCode::Down, KeyModifiers::ALT);
    let alt_page_up = KeyEvent::new(KeyCode::PageUp, KeyModifiers::ALT);
    let alt_page_down = KeyEvent::new(KeyCode::PageDown, KeyModifiers::ALT);

    assert_eq!(resolve_key_event(alt_up), InputAction::ScrollUp(1));
    assert_eq!(resolve_key_event(alt_down), InputAction::ScrollDown(1));
    assert_eq!(resolve_key_event(alt_page_up), InputAction::ScrollUp(8));
    assert_eq!(resolve_key_event(alt_page_down), InputAction::ScrollDown(8));
}

#[test]
fn question_mark_is_inserted_not_help() {
    let question = KeyEvent::new(KeyCode::Char('?'), KeyModifiers::SHIFT);
    assert_eq!(resolve_key_event(question), InputAction::Insert('?'));
}

#[test]
fn custom_keybindings_support_autocomplete_and_send() {
    let config = KeybindingsConfig {
        send: String::from("Alt+Enter"),
        autocomplete: String::from("Tab"),
        ..KeybindingsConfig::default()
    };
    let bindings = InputBindings::from_config(&config);
    let alt_enter = KeyEvent::new(KeyCode::Enter, KeyModifiers::ALT);
    let tab = KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE);

    assert_eq!(
        resolve_key_event_with_bindings(alt_enter, &bindings, "ignored"),
        InputAction::Send
    );
    assert_eq!(
        resolve_key_event_with_bindings(tab, &bindings, "/ex"),
        InputAction::AutoCompleteCommand
    );
}
