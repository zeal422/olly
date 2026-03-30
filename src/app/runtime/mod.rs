pub mod confirm_delete;
pub mod event_loop;
pub mod key_actions;
pub mod overlays;
pub mod shutdown;
pub mod startup;

use ratatui::{backend::CrosstermBackend, Terminal};

pub(crate) type AppTerminal = Terminal<CrosstermBackend<std::io::Stdout>>;
