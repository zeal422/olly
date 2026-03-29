#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Action {
    Send,
    NewLine,
}

fn resolve_input_key(ctrl: bool, key: &str) -> Action {
    if ctrl && key == "Enter" {
        Action::Send
    } else {
        Action::NewLine
    }
}

#[test]
fn ctrl_enter_sends_enter_adds_newline() {
    assert_eq!(resolve_input_key(true, "Enter"), Action::Send);
    assert_eq!(resolve_input_key(false, "Enter"), Action::NewLine);
}
