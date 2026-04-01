# TO FIX

## [P1] Selection/copy can use stale previous-chat data in non-chat states

- **Location**
  - `src/app/tui/mod.rs:1128` (welcome early return)
  - `src/app/tui/mod.rs:1195` (no-session early return)
  - `src/app/tui/mod.rs:1229` (`update_chat_area_metadata` only in normal chat path)
  - `src/app/runtime/key_actions.rs:22` (`mouse_to_chat_visual_position`)
  - `src/app/runtime/key_actions.rs:265` (`handle_copy`)
- **Impact**
  - When UI switches to welcome/no-session views, chat metadata can remain from a previous session.
  - Mouse selection/copy can read stale `chat_visual_lines` and copy old content.
- **Fix direction**
  - Reset or refresh chat metadata (`chat_area`, `chat_visual_lines`, scroll hint, selection) before early returns in non-chat views.

## [P1] Mouse column mapping is wrong for right/center-aligned chat lines

- **Location**
  - `src/app/tui/mod.rs:773` (`Alignment::Right` for user lines)
  - `src/app/tui/mod.rs:799` (`Alignment::Center` for system lines)
  - `src/app/runtime/key_actions.rs:35` (`col_index = col - area.x` without alignment compensation)
- **Impact**
  - Selection anchor/current columns are computed as if all lines are left-aligned.
  - For right/center-aligned lines, click/drag can select incorrect characters or empty ranges.
- **Fix direction**
  - Convert mouse X to line-relative character index using the rendered line width + alignment offset for the specific visual line.
