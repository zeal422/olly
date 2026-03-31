use super::helpers::*;

#[test]
fn input_backspace_at_start_does_not_panic() {
    let mut app = App::new(
        String::from("http://localhost:11434"),
        String::from("llama3.2:latest"),
    );

    app.set_input_text(String::new());
    app.backspace_input_char(); // Should not panic

    assert_eq!(app.input, "");
}

#[test]
fn input_insert_newline_resets_caret_to_line_start() {
    let mut app = App::new(
        String::from("http://localhost:11434"),
        String::from("llama3.2:latest"),
    );

    app.set_input_text(String::from("hello world"));
    app.insert_input_char('\n');

    assert_eq!(app.input, "hello world\n");
    // After newline, cursor should be at start of new line (position after '\n')
    assert_eq!(app.input_cursor_char_index, app.input.chars().count());
}

#[test]
fn input_cursor_left_moves_back_one_char() {
    let mut app = App::new(
        String::from("http://localhost:11434"),
        String::from("llama3.2:latest"),
    );

    app.set_input_text(String::from("abc"));
    app.input_cursor_char_index = 2; // past 'b'

    app.move_input_cursor_left();

    assert_eq!(app.input_cursor_char_index, 1);
}

#[test]
fn input_cursor_right_moves_forward_one_char() {
    let mut app = App::new(
        String::from("http://localhost:11434"),
        String::from("llama3.2:latest"),
    );

    app.set_input_text(String::from("abc\ndef"));
    app.input_cursor_char_index = 2; // before 'c'

    app.move_input_cursor_right();

    assert_eq!(app.input_cursor_char_index, 3); // now on '\n'
}

#[test]
fn input_vertical_movement_preserves_preferred_column() {
    let mut app = App::new(
        String::from("http://localhost:11434"),
        String::from("llama3.2:latest"),
    );

    // Create a multi-line input where cursor is in middle of first line
    app.set_input_text(String::from("12345\n67890\nabcde"));
    app.input_cursor_char_index = 2; // on '3'
    app.move_input_cursor_down(); // move to second line

    // Preferred column should be 2 (original column)
    assert_eq!(app.input_cursor_preferred_column, Some(2));

    // Now move back up
    app.move_input_cursor_up();

    // Should restore to column 2 on first line
    assert_eq!(app.input_cursor_char_index, 2);
}

#[test]
fn input_long_line_editing_maintains_cursor_position() {
    let mut app = App::new(
        String::from("http://localhost:11434"),
        String::from("llama3.2:latest"),
    );

    // Create a long line
    let long_text = "a".repeat(200);
    app.set_input_text(long_text.clone());

    // Move cursor to position 100
    app.input_cursor_char_index = 100;
    let before_len = app.input.len();

    // Backspace: deletes char before cursor (index 99) and moves cursor back
    app.backspace_input_char();
    assert_eq!(app.input.len(), before_len - 1);
    assert_eq!(app.input_cursor_char_index, 99);

    // Verify the character that was at index 100 is now at index 99
    // (since everything shifted left)
    // The char at index 99 originally was 'a', still 'a' (the one that was at 100 shifted left)
    // Actually after backspace, the char that was at index 100 moves to index 99
    // But we don't need to check that; just that length and cursor are correct

    // Insert a char at cursor
    app.insert_input_char('X');
    assert_eq!(app.input.len(), before_len);
    assert_eq!(app.input_cursor_char_index, 100);
    // Verify X is at index 99 (where cursor was when inserted)
    assert_eq!(app.input.chars().nth(99).unwrap(), 'X');
}
