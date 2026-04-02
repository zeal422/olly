# Workflow

How Olly processes input, renders output, and manages state through its event loop.

## Application Lifecycle

```mermaid
flowchart TD
    Start[main.rs: CLI parsing] --> Bootstrap[runtime/startup.rs: bootstrap]
    Bootstrap --> LoadConfig[Load config.toml]
    LoadConfig --> InitTerminal[Enable raw mode, mouse capture]
    InitTerminal --> LoadUIState[Load ui_state.json]
    LoadUIState --> ApplyState[Apply saved UI state with startup precedence]
    ApplyState --> RefreshModels[Refresh model list from Ollama]
    RefreshModels --> EventLoop[runtime/event_loop.rs: run]
    EventLoop --> Shutdown[runtime/shutdown.rs: teardown]
    Shutdown --> SaveState[Persist ui_state.json]
    SaveState --> RestoreTerminal[Restore terminal mode]
    RestoreTerminal --> End[Exit]
```

## Event Loop

The event loop runs at a fixed tick rate of 80ms (12.5 Hz) and processes three types of events:

```mermaid
stateDiagram-v2
    [*] --> WaitForEvent
    WaitForEvent --> KeyPressed: Keyboard event
    WaitForEvent --> MouseAction: Mouse event
    WaitForEvent --> Tick: 80ms timeout
    WaitForEvent --> Resize: Terminal resize

    KeyPressed --> ResolveAction: resolve_key_event_with_bindings
    MouseAction --> ResolveAction: resolve_mouse_event
    Resize --> HandleResize: Update metadata

    ResolveAction --> CheckOverlay: Action != Noop?
    CheckOverlay --> OverlayConsume: Overlay active?
    CheckOverlay --> HandleAction: No overlay or not consumed

    OverlayConsume --> WaitForEvent: Action consumed
    HandleAction --> WaitForAction: Action handled

    Tick --> ExpireToast: Check toast expiration
    Tick --> ExpireConfirmations: Check confirmation timeouts
    Tick --> AutoScroll: Check auto-scroll direction
    Tick --> UpdateSpinner: Animate loading spinner
    Tick --> WaitForEvent

    ExpireToast --> WaitForEvent
    ExpireConfirmations --> WaitForEvent
    AutoScroll --> WaitForEvent
    UpdateSpinner --> WaitForEvent
    HandleResize --> WaitForEvent
```

## Input Resolution Pipeline

```mermaid
flowchart LR
    subgraph "Raw Input"
        KeyEvent[crossterm KeyEvent]
        MouseEvent[crossterm MouseEvent]
    end

    subgraph "Resolution"
        KeyResolve[resolve_key_event_with_bindings]
        MouseResolve[resolve_mouse_event]
    end

    subgraph "InputAction"
        Send[Send]
        NewLine[NewLine]
        Quit[Quit]
        ScrollUp[ScrollUp]
        ScrollDown[ScrollDown]
        Left[CursorLeft]
        Right[CursorRight]
        Up[CursorUp]
        Down[CursorDown]
        Paste[Paste]
        Copy[Copy]
        StartSel[StartSelection]
        UpdateSel[UpdateSelection]
        ExtendSel[ExtendSelection]
        EndSel[EndSelection]
        Dismiss[DismissOverlay]
        ToggleHelp[ToggleHelp]
        Noop[Noop]
    end

    KeyEvent --> KeyResolve
    MouseEvent --> MouseResolve

    KeyResolve --> Send
    KeyResolve --> NewLine
    KeyResolve --> Quit
    KeyResolve --> ScrollUp
    KeyResolve --> ScrollDown
    KeyResolve --> Left
    KeyResolve --> Right
    KeyResolve --> Up
    KeyResolve --> Down
    KeyResolve --> Paste
    KeyResolve --> Copy
    KeyResolve --> Dismiss
    KeyResolve --> ToggleHelp
    KeyResolve --> Noop

    MouseResolve --> Paste
    MouseResolve --> StartSel
    MouseResolve --> UpdateSel
    MouseResolve --> ExtendSel
    MouseResolve --> EndSel
    MouseResolve --> ScrollUp
    MouseResolve --> ScrollDown
    MouseResolve --> Noop
```

## Action Handling

```mermaid
flowchart TD
    Action[InputAction] --> Overlay{Overlay active?}

    Overlay -->|Yes| OverlayHandler[overlay handler]
    Overlay -->|No| Dispatch[handle_action dispatch]

    OverlayHandler --> DismissOverlay[Esc dismisses]
    OverlayHandler --> OverlayScroll[ScrollUp/Down for picker]
    OverlayHandler --> OverlayNav[Left/Right/Enter for picker]

    Dispatch --> ChatFocus{Focus pane?}

    ChatFocus -->|Chat| ChatActions[Send, NewLine, Scroll, Selection, Copy, Paste]
    ChatFocus -->|Workspaces| SidebarActions[Sidebar navigation, Activate selection]

    ChatActions --> KeyActionHandler[key_actions.rs handlers]
    SidebarActions --> KeyActionHandler

    KeyActionHandler --> SlashCommand{Starts with /?}
    SlashCommand -->|Yes| CommandDispatch[handle_command]
    SlashCommand -->|No| DirectAction[Direct action execution]

    CommandDispatch --> RouteCommand[commands::route_command]
    RouteCommand --> UtilityHandler[utility handler]
    RouteCommand --> ModelsHandler[models handler]
    RouteCommand --> MiscHandler[misc handler]
    RouteCommand --> WorkspaceHandler[workspace handler]
    RouteCommand --> SessionHandler[session handler]
    RouteCommand --> UnknownHandler[Unknown command fallback]

    DirectAction --> MutateState[App state mutation]
    UtilityHandler --> MutateState
    ModelsHandler --> MutateState
    MiscHandler --> MutateState
    WorkspaceHandler --> MutateState
    SessionHandler --> MutateState
    UnknownHandler --> MutateState

    MutateState --> Redraw[Frame redraw]
```

## Chat Send Pipeline

```mermaid
sequenceDiagram
    participant Input as User Input
    participant Pipeline as chat_pipeline.rs
    participant State as App State
    participant Ollama as Ollama API
    participant TUI as TUI Renderer
    participant SQLite as SQLite

    Input->>Pipeline: take_input_for_send()
    Pipeline->>State: push_user(message)
    State->>SQLite: Persist user message
    Pipeline->>Ollama: Stream chat request
    Ollama-->>Pipeline: Token chunk
    Pipeline->>TUI: render_chat_text (live update)
    Ollama-->>Pipeline: Token chunk
    Pipeline->>TUI: render_chat_text (live update)
    Ollama-->>Pipeline: Done + usage stats
    Pipeline->>State: Update token telemetry
    Pipeline->>State: push_assistant(response)
    State->>SQLite: Persist assistant message
    Pipeline->>State: persist_active_session_context_snapshot()
```

## Text Selection Workflow

```mermaid
stateDiagram-v2
    [*] --> Idle: No selection

    Idle --> Anchored: Left mouse down on chat
    Idle --> Anchored: Shift+click on chat (no anchor)

    Anchored --> Dragging: Mouse drag
    Anchored --> Selected: Shift+click (with anchor)

    Dragging --> Dragging: Continue drag
    Dragging --> AutoScrollUp: Mouse near top edge
    Dragging --> AutoScrollDown: Mouse near bottom edge
    Dragging --> Selected: Mouse release

    AutoScrollUp --> Dragging: Mouse moves away from edge
    AutoScrollDown --> Dragging: Mouse moves away from edge

    Selected --> Copied: Ctrl+C
    Selected --> Idle: Scroll action
    Selected --> Idle: New message
    Selected --> Idle: Focus change
    Selected --> Idle: Resize event
    Selected --> Idle: Clear selection

    Copied --> Idle: Selection cleared after copy

    note right of AutoScrollUp
        Auto-scroll runs in tick() at 80ms
        Uses scroll_up_preserve_selection
        to avoid clearing selection state
    end note
```

## Rendering Cycle

```mermaid
flowchart TD
    Draw[draw function] --> CheckSize{Frame size >= 2x2?}
    CheckSize -->|No| Return[Return early]
    CheckSize -->|Yes| BuildPalette[Build ThemePalette]
    BuildPalette --> CalcLayout[Calculate root/body/input areas]
    CalcLayout --> RenderStatus[render_status_bar]
    RenderStatus --> CheckWidth{Width < 115?}
    CheckWidth -->|Yes| VerticalLayout[Vertical split: sidebar above chat]
    CheckWidth -->|No| HorizontalLayout[Horizontal split: sidebar left, chat right]
    VerticalLayout --> RenderSidebar[render_models_panel]
    HorizontalLayout --> RenderSidebar
    RenderSidebar --> RenderChat[render_chat_panel]
    RenderChat --> RenderInput[render_input_panel]
    RenderInput --> CheckToast{Toast active?}
    CheckToast -->|Yes| RenderToast[Render toast widget]
    CheckToast -->|No| CheckModal
    RenderToast --> CheckModal{Modal active?}
    CheckModal -->|Yes| RenderModal[render_help/workspace/session/theme/tool modals]
    CheckModal -->|No| FrameDone
    RenderModal --> FrameDone[Frame complete]
```

## Persistence Flow

```mermaid
flowchart TD
    Startup[App startup] --> LoadConfig[Load config.toml]
    LoadConfig --> LoadUIState[Load ui_state.json]
    LoadUIState --> ApplySavedState[apply_saved_ui_state_with_startup_precedence]
    ApplySavedState --> CheckModelOverride{Model overridden via CLI?}
    CheckModelOverride -->|Yes| OverrideModel[Use CLI model]
    CheckModelOverride -->|No| UseSavedModel[Use saved model]
    OverrideModel --> RefreshWorkspaces
    UseSavedModel --> RefreshWorkspaces[Refresh workspaces from SQLite]
    RefreshWorkspaces --> RestoreSession{Restored session context matches?}
    RestoreSession -->|Yes| RestoreTokens[Restore token counts]
    RestoreSession -->|No| FreshStart[Fresh start]
    RestoreTokens --> EventLoop
    FreshStart --> EventLoop

    Shutdown[App shutdown] --> PersistUIState[save_ui_state_to_path]
    PersistUIState --> RestoreTerminal[Restore terminal mode]
```

## Tick Processing

Every 80ms, the following occurs:

1. **Spinner animation**: `spinner_frame` increments for loading indicator
2. **Toast expiration**: Checks if `toast.expires_at` has passed
3. **Confirmation expiration**: Checks if pending modal timeouts have expired
4. **Auto-scroll**: If `auto_scroll_direction` is set during drag selection, applies scroll while preserving selection state
