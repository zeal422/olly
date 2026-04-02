# ASCII Preloader Implementation Plan

## Objective

Add an animated ASCII "OLLY" logo preloader that displays immediately on app launch while bootstrap (config, SQLite, Ollama network calls) runs in the background. Eliminates the blank-screen startup experience.

---

## Current Startup Problem

**Current order:**
```
bootstrap() [sync DB + async HTTP] → init_terminal() → event_loop()
```

The user sees a blank terminal during the entire bootstrap phase (~100-400ms, or seconds if Ollama is slow).

**Bootstrap steps that block:**
1. `AppConfig::load_or_create()` — file I/O (~5ms)
2. `WorkspaceManager::open()` — SQLite open (~5ms)
3. `app.refresh_workspaces()` — DB query (~5ms)
4. `load_conversation_into_app()` — DB query (~10ms)
5. `list_models()` — **HTTP GET to Ollama** (~50-500ms, network-dependent)
6. `refresh_model_context_window()` — **HTTP GET to Ollama** (~50-500ms)

**Terminal init happens AFTER all of this completes.**

---

## Solution

**New order:**
```
init_terminal() → render preloader → bootstrap() async → transition to real UI → event_loop()
```

The terminal is initialized first. A preloader frame renders immediately. Bootstrap runs in the background. When complete, the UI transitions seamlessly.

---

## ASCII Art Frames

the logo:

    ███████    █████       █████       █████ █████
  ███░░░░░███ ░░███       ░░███       ░░███ ░░███ 
 ███     ░░███ ░███        ░███        ░░███ ███  
░███      ░███ ░███        ░███         ░░█████   
░███      ░███ ░███        ░███          ░░███    
░░███     ███  ░███      █ ░███      █    ░███    
 ░░░███████░   ███████████ ███████████    █████   
   ░░░░░░░    ░░░░░░░░░░░ ░░░░░░░░░░░    ░░░░░    

The "OLLY" logo uses block characters (█/░) for a wave animation effect. 4 frames cycle at the 80ms tick rate:

```
Frame 0:                          Frame 1:
    ███████    █████       █████       █████ █████    ███░░░░░███ ░░███       ░░███       ░░███ ░░███ 
  ███░░░░░███ ░░███       ░░███       ░░███ ░░███   ███     ░░███ ░███        ░███        ░░███ ███  
 ███     ░░███ ░███        ░███        ░░███ ███    ░███      ░███ ░███        ░███         ░░█████   
░███      ░███ ░███        ░███         ░░█████     ░███      ░███ ░███        ░███          ░░███    
░███      ░███ ░███        ░███          ░░███      ░░███     ███  ░███      █ ░███      █    ░███    
░░███     ███  ░███      █ ░███      █    ░███       ░░░███████░   ███████████ ███████████    █████   
 ░░░███████░   ███████████ ███████████    █████         ░░░░░░░    ░░░░░░░░░░░ ░░░░░░░░░░░    ░░░░░    
   ░░░░░░░    ░░░░░░░░░░░ ░░░░░░░░░░░    ░░░░░                                             

Frame 2:                          Frame 3:
░░░░░░░███ ░███        ░███        ░░███ ███    ██░░░░░░░░ ░███        ░███        ░░███ ░░███ 
░░███ ░░███ ░███        ░███         ░░█████    ░░███      ░███        ░███         ░░█████   
  ░░█████   ░███        ░███          ░░███      ░░███     ███  ░███      █ ░███      █    ░███ 
    ░░███   ░███      █ ░███      █    ░███       ░░░███████░   ███████████ ███████████    █████ 
     ░███    ███████████ ███████████    █████         ░░░░░░░    ░░░░░░░░░░░ ░░░░░░░░░░░    ░░░░░    
     ░███    ░░░░░░░░░░░ ░░░░░░░░░░░    ░░░░░                                             
    █████                                                                                  
   ░░░░░                                                                                   
```

Below the logo: a status text that updates during bootstrap stages.

---

## Phase 1: Reorder Startup (`src/app/mod.rs`)

### Changes

Move `init_terminal()` **before** `bootstrap()`. The function `run()` currently does:

```rust
// Current (problematic):
let bootstrap = bootstrap(url_override, model_override).await?;
let mut terminal = init_terminal()?;
```

Change to:

```rust
// New (preloader):
let mut terminal = init_terminal()?;
let bootstrap = bootstrap_with_progress(url_override, model_override, |stage| {
    // Update app status for preloader to display
}).await?;
```

### Implementation

The `run()` function will:
1. Initialize terminal immediately
2. Create a minimal `App` with `is_loading = true`
3. Enter a preloader render loop that calls `terminal.draw(|f| render_preloader(f, &app))`
4. Run `bootstrap()` async in the background (using `tokio::spawn` or sequential with periodic preloader renders)
5. When bootstrap completes, populate the real `App` and enter the event loop

---

## Phase 2: Add Preloader Render Function (`src/app/tui/mod.rs`)

### New Function

```rust
pub fn render_preloader(frame: &mut Frame<'_>, app: &App) {
    let area = frame.area();
    
    // ASCII frames
    const FRAMES: &[&str] = &[/* 4 frames */];
    let frame_idx = app.spinner_frame % FRAMES.len();
    let logo = FRAMES[frame_idx];
    
    // Status text
    let status = if app.status.is_empty() { "Loading..." } else { &app.status };
    
    // Center logo + status in terminal
    let logo_lines: Vec<&str> = logo.lines().collect();
    let logo_height = logo_lines.len() as u16;
    let logo_width = logo_lines.iter().map(|l| l.chars().count()).max().unwrap_or(0) as u16;
    
    let y_offset = (area.height.saturating_sub(logo_height + 3)) / 2;
    let x_offset = (area.width.saturating_sub(logo_width)) / 2;
    
    for (i, line) in logo_lines.iter().enumerate() {
        let span = Span::styled(*line, Style::default().fg(palette.chat_fg));
        let line = Line::from(span);
        frame.render_widget(line, Rect::new(x_offset, y_offset + i as u16, area.width, 1));
    }
    
    // Status text below logo
    let status_y = y_offset + logo_height + 1;
    let status_span = Span::styled(status, Style::default().fg(palette.body_muted));
    frame.render_widget(Line::from(status_span), Rect::new(0, status_y, area.width, 1));
}
```

### Frame Constants

The 4 frames will be stored as `const FRAMES: &[&str]` in `tui/mod.rs`. Each frame is the full OLly ASCII art with a different █/░ wave pattern.

---

## Phase 3: Bootstrap with Progress Callback (`src/app/runtime/startup.rs`)

### Changes

Modify `bootstrap()` to accept a progress callback:

```rust
pub(crate) async fn bootstrap_with_progress<F>(
    url_override: Option<String>,
    model_override: Option<String>,
    mut on_progress: F,
) -> Result<RuntimeBootstrap, Box<dyn std::error::Error>>
where
    F: FnMut(&str),
{
    on_progress("Loading config...");
    let config = AppConfig::load_or_create()?;
    
    on_progress("Loading workspace...");
    let manager = WorkspaceManager::open(Path::new(&config.storage.db_path))?;
    // ... workspace/session loading ...
    
    on_progress("Connecting to Ollama...");
    match crate::ollama::models::list_models(&client, &app.url).await {
        // ...
    }
    
    on_progress("Loading model info...");
    refresh_model_context_window(&mut app, &client).await;
    
    on_progress("Ready");
    Ok(RuntimeBootstrap { ... })
}
```

### Progress Integration

The callback updates `app.status` which the preloader reads each frame. Since bootstrap runs async and the preloader render loop runs on the main thread, we need a channel or shared state to communicate progress.

**Approach**: Use `tokio::sync::watch` channel:
- Bootstrap task sends progress updates via `watch::Sender`
- Preloader loop receives via `watch::Receiver` and updates `app.status`
- No locking, no race conditions

---

## Phase 4: Transition (`src/app/mod.rs`)

### Changes

When bootstrap completes:
1. Stop the preloader render loop
2. Populate the real `App` from `RuntimeBootstrap`
3. Enter the normal event loop

```rust
// After bootstrap completes:
let RuntimeBootstrap { app, manager, session, client, input_bindings, config, ui_state_path } = bootstrap?;
let last_saved_ui_state = persist_initial_ui_state(&app, &ui_state_path);

// Enter event loop (normal rendering):
runtime::event_loop::run(&mut app, &manager, &mut session, &config.defaults.model, &client, &input_bindings, &mut terminal, &ui_state_path, &mut last_saved_ui_state).await?;
```

The transition is instant — the next `terminal.draw()` call renders the real UI instead of the preloader.

---

## File Touch List

| File | Changes |
|------|---------|
| `src/app/mod.rs` | Reorder startup, add preloader render loop, transition logic |
| `src/app/runtime/startup.rs` | Add `bootstrap_with_progress()`, progress callback |
| `src/app/tui/mod.rs` | Add `render_preloader()`, ASCII frame constants |

---

## Risks & Mitigations

| Risk | Impact | Mitigation |
|------|--------|------------|
| Terminal cleanup on bootstrap failure | App leaves terminal in raw mode | Existing `teardown_terminal()` in shutdown handles this; add try/finally around bootstrap |
| Preloader renders before terminal is ready | Garbled output | `init_terminal()` completes before any render calls |
| Progress callback blocks bootstrap | Slower startup | Use `watch` channel (lock-free, non-blocking) |
| ASCII art doesn't render correctly on all terminals | Visual glitch | Block characters (█/░) are widely supported; test on Windows Terminal, Linux, macOS |
| Preloader flickers during transition | Poor UX | Transition happens on next frame — no intermediate state visible |

---

## Definition of Done

- [ ] Terminal initializes before bootstrap
- [ ] Animated ASCII "OLLY" logo displays immediately on launch
- [ ] Logo cycles through 4 wave-animation frames at 80ms tick rate
- [ ] Bootstrap stage text updates below logo ("Loading config...", "Connecting to Ollama...", etc.)
- [ ] Seamless transition from preloader to real UI when bootstrap completes
- [ ] No flicker or visual artifacts during transition
- [ ] Bootstrap failure shows error toast, not blank screen
- [ ] All quality gates pass: `cargo fmt`, `cargo clippy -D warnings`, `cargo test`
- [ ] Manual smoke test: launch confirms preloader → real UI transition

---

## Estimated Effort

- Phase 1 (reorder startup): ~30 minutes
- Phase 2 (preloader render): ~45 minutes
- Phase 3 (progress callback): ~30 minutes
- Phase 4 (transition): ~15 minutes
- Testing & polish: ~30 minutes

**Total: ~2.5 hours**



