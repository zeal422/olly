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

The base logo (provided by user):

```
    ███████    █████       █████       █████ █████
  ███░░░░░███ ░░███       ░░███       ░░███ ░░███ 
 ███     ░░███ ░███        ░███        ░░███ ███  
░███      ░███ ░███        ░███         ░░█████   
░███      ░███ ░███        ░███          ░░███    
░░███     ███  ░███      █ ░███      █    ░███    
 ░░░███████░   ███████████ ███████████    █████   
   ░░░░░░░    ░░░░░░░░░░░ ░░░░░░░░░░░    ░░░░░    
```

**Animation strategy**: 4 frames that cycle at 80ms tick rate. Each frame shifts the ░/█ boundary to create a flowing wave effect across the "OLLY" text. The frames are stored as `const FRAMES: &[&str]` in `tui/mod.rs`.

Below the logo: a status text that updates during bootstrap stages:
- "Loading config..."
- "Loading workspace..."
- "Connecting to Ollama..."
- "Loading model info..."
- "Ready"

---

## Phase 1: Reorder Startup (`src/app/mod.rs`)

### Current `run()` function (src/app/mod.rs:75-100):

```rust
pub async fn run(
    url_override: Option<String>,
    model_override: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut runtime = runtime::startup::bootstrap(url_override, model_override).await?;
    let mut terminal = runtime::startup::init_terminal()?;
    let mut last_saved_ui_state =
        runtime::startup::persist_initial_ui_state(&runtime.app, &runtime.ui_state_path);

    let run_result = runtime::event_loop::run(
        &mut runtime.app, &mut runtime.manager, &mut runtime.session,
        &runtime.config.defaults.model, &runtime.client, &runtime.input_bindings,
        &mut terminal, &runtime.ui_state_path, &mut last_saved_ui_state,
    ).await;

    runtime::shutdown::teardown_terminal(&mut terminal, &runtime.app, &runtime.ui_state_path)?;
    run_result
}
```

### New `run()` function:

```rust
pub async fn run(
    url_override: Option<String>,
    model_override: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    // 1. Initialize terminal FIRST — user sees preloader immediately
    let mut terminal = runtime::startup::init_terminal()?;

    // 2. Create progress channel for bootstrap status updates
    let (progress_tx, mut progress_rx) = tokio::sync::watch::channel(String::from("Initializing..."));

    // 3. Spawn bootstrap in background
    let bootstrap_handle = tokio::spawn(async move {
        runtime::startup::bootstrap_with_progress(url_override, model_override, |stage| {
            let _ = progress_tx.send(stage.to_string());
        }).await
    });

    // 4. Preloader render loop while bootstrap runs
    while !bootstrap_handle.is_finished() {
        // Check for progress updates
        let _ = progress_rx.borrow_and_update();
        let status = progress_rx.borrow().clone();

        // Render preloader frame
        terminal.draw(|f| crate::app::tui::render_preloader(f, &status))?;

        // Wait for tick interval
        tokio::time::sleep(Duration::from_millis(80)).await;
    }

    // 5. Bootstrap complete — get result
    let mut runtime = bootstrap_handle.await??;

    // 6. Persist UI state and enter event loop
    let mut last_saved_ui_state =
        runtime::startup::persist_initial_ui_state(&runtime.app, &runtime.ui_state_path);

    let run_result = runtime::event_loop::run(
        &mut runtime.app, &mut runtime.manager, &mut runtime.session,
        &runtime.config.defaults.model, &runtime.client, &runtime.input_bindings,
        &mut terminal, &runtime.ui_state_path, &mut last_saved_ui_state,
    ).await;

    runtime::shutdown::teardown_terminal(&mut terminal, &runtime.app, &runtime.ui_state_path)?;
    run_result
}
```

### Key Changes:
- `init_terminal()` moves before `bootstrap()`
- Bootstrap runs via `tokio::spawn` in background
- Preloader render loop runs while bootstrap is in progress
- Progress communicated via `tokio::sync::watch` channel (lock-free, non-blocking)
- Terminal teardown on bootstrap failure handled by try/finally pattern

---

## Phase 2: Add Preloader Render Function (`src/app/tui/mod.rs`)

### New Function:

```rust
use ratatui::text::{Line, Span};
use ratatui::layout::Rect;
use ratatui::style::{Style, Color};

pub fn render_preloader(frame: &mut Frame<'_>, status: &str) {
    let area = frame.area();
    
    // ASCII frames for wave animation
    const FRAMES: &[&str] = &[
        // Frame 0: full logo
        "    ███████    █████       █████       █████ █████\n  ███░░░░░███ ░░███       ░░███       ░░███ ░░███\n ███     ░░███ ░███        ░███        ░░███ ███\n░███      ░███ ░███        ░███         ░░█████\n░███      ░███ ░███        ░███          ░░███\n░░███     ███  ░███      █ ░███      █    ░███\n ░░░███████░   ███████████ ███████████    █████\n   ░░░░░░░    ░░░░░░░░░░░ ░░░░░░░░░░░    ░░░░░",
        // Frame 1: wave shift left
        // Frame 2: wave shift center
        // Frame 3: wave shift right
        // (actual frames designed during implementation)
    ];
    
    // Cycle frames using a global counter (passed via status or app state)
    // Since we don't have app state in preloader, use a static counter
    static FRAME_COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let frame_idx = FRAME_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed) % FRAMES.len();
    let logo = FRAMES[frame_idx];
    
    // Center logo in terminal
    let logo_lines: Vec<&str> = logo.lines().collect();
    let logo_height = logo_lines.len() as u16;
    let logo_width = logo_lines.iter().map(|l| l.chars().count()).max().unwrap_or(0) as u16;
    
    let y_offset = area.height.saturating_sub(logo_height + 3) / 2;
    let x_offset = area.width.saturating_sub(logo_width) / 2;
    
    // Render each line of the logo
    for (i, line) in logo_lines.iter().enumerate() {
        let span = Span::styled(*line, Style::default().fg(Color::Cyan));
        let line_widget = Line::from(span);
        frame.render_widget(line_widget, Rect::new(x_offset, y_offset + i as u16, area.width, 1));
    }
    
    // Render status text below logo
    let status_y = y_offset + logo_height + 1;
    let status_span = Span::styled(status, Style::default().fg(Color::DarkGray));
    let status_line = Line::from(status_span);
    frame.render_widget(status_line, Rect::new(0, status_y, area.width, 1));
}
```

### Design Notes:
- Uses `Color::Cyan` for logo (visible on dark/light themes)
- Uses `Color::DarkGray` for status text
- Static atomic counter for frame cycling (no App state needed during preloader)
- Logo centered both horizontally and vertically

---

## Phase 3: Bootstrap with Progress Callback (`src/app/runtime/startup.rs`)

### New Function:

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
    let ui_state_path = ui_state_path_for_config(&config);
    
    let url = url_override.unwrap_or_else(|| config.ollama.url.clone());
    let model_was_overridden = model_override.is_some();
    let startup_model = model_override.unwrap_or_else(|| config.defaults.model.clone());
    let mut app = App::new(url, startup_model.clone());
    app.workspace_name = config.defaults.workspace.clone();
    app.syntax_highlighting = config.ui.syntax_highlighting;
    
    if let Some(saved_ui_state) = load_ui_state_from_path(&ui_state_path) {
        apply_saved_ui_state_with_startup_precedence(
            &mut app, saved_ui_state, &startup_model, model_was_overridden,
        );
    }
    
    let input_bindings = crate::app::input::InputBindings::from_config(&config.keybindings);
    
    on_progress("Loading workspace...");
    let manager = WorkspaceManager::open(Path::new(&config.storage.db_path))?;
    let mut session = WorkspaceSession { workspace_id: 0, conversation_id: 0, preset_id: 0 };
    
    app.refresh_workspaces(&manager)?;
    // ... workspace/session loading (same as current bootstrap) ...
    
    on_progress("Connecting to Ollama...");
    let client = reqwest::Client::new();
    match crate::ollama::models::list_models(&client, &app.url).await {
        Ok(models) => {
            app.connection_status = ConnectionStatus::Online;
            // ... model handling ...
        }
        Err(err) => {
            app.connection_status = ConnectionStatus::Offline;
        }
    }
    
    on_progress("Loading model info...");
    app.restore_session_context_snapshot_if_match();
    refresh_model_context_window(&mut app, &client).await;
    
    on_progress("Ready");
    
    Ok(RuntimeBootstrap {
        config, ui_state_path, app, manager, session, client, input_bindings,
    })
}
```

### Changes from current `bootstrap()`:
- Same logic, just adds `on_progress()` calls at key points
- No structural changes to data flow
- Backward compatible — existing `bootstrap()` can call `bootstrap_with_progress()` with a no-op callback

---

## Phase 4: Terminal Teardown on Bootstrap Failure

### Error Handling:

If bootstrap fails after terminal is initialized, we must restore the terminal:

```rust
// In run():
let bootstrap_result = tokio::spawn(async move { ... }).await;

let mut runtime = match bootstrap_result {
    Ok(Ok(r)) => r,
    Ok(Err(e)) => {
        // Bootstrap failed — teardown terminal before propagating error
        runtime::shutdown::teardown_terminal(&mut terminal, &App::new(String::new(), String::new()), &ui_state_path)?;
        return Err(e);
    }
    Err(e) => {
        // Spawn failed (panic/cancelled)
        runtime::shutdown::teardown_terminal(&mut terminal, &App::new(String::new(), String::new()), &ui_state_path)?;
        return Err(Box::new(e));
    }
};
```

---

## File Touch List

| File | Changes |
|------|---------|
| `src/app/mod.rs` | Reorder `run()`: init_terminal first, spawn bootstrap, preloader loop, transition |
| `src/app/runtime/startup.rs` | Add `bootstrap_with_progress()` with progress callbacks |
| `src/app/tui/mod.rs` | Add `render_preloader()` with ASCII frame constants |

---

## Risks & Mitigations

| Risk | Impact | Mitigation |
|------|--------|------------|
| Terminal cleanup on bootstrap failure | App leaves terminal in raw mode | Explicit teardown in error paths (Phase 4) |
| Preloader renders before terminal is ready | Garbled output | `init_terminal()` completes before any render calls |
| Progress channel blocks bootstrap | Slower startup | `watch` channel is lock-free, non-blocking |
| ASCII art doesn't render correctly on all terminals | Visual glitch | Block characters (█/░) are widely supported; test on Windows Terminal, Linux, macOS |
| Preloader flickers during transition | Poor UX | Transition happens on next frame — no intermediate state visible |
| Static frame counter overflows | Frame cycle breaks | `AtomicUsize` wraps at usize::MAX — won't happen in practice |

---

## Definition of Done

- [ ] Terminal initializes before bootstrap
- [ ] Animated ASCII "OLLY" logo displays immediately on launch
- [ ] Logo cycles through 4 wave-animation frames at 80ms tick rate
- [ ] Bootstrap stage text updates below logo
- [ ] Seamless transition from preloader to real UI when bootstrap completes
- [ ] No flicker or visual artifacts during transition
- [ ] Bootstrap failure restores terminal cleanly
- [ ] All quality gates pass: `cargo fmt`, `cargo clippy -D warnings`, `cargo test`
- [ ] Manual smoke test: launch confirms preloader → real UI transition

---

## Estimated Effort

- Phase 1 (reorder startup): ~30 minutes
- Phase 2 (preloader render): ~45 minutes
- Phase 3 (progress callback): ~30 minutes
- Phase 4 (error handling): ~15 minutes
- Testing & polish: ~30 minutes

**Total: ~2.5 hours**
