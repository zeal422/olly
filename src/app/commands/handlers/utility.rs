use std::{fs, path::PathBuf};

use base64::{engine::general_purpose::STANDARD, Engine as _};

use crate::app::{
    commands::CommandRoute,
    state::{PendingImage, ToastLevel},
    App, COMMAND_SUGGESTIONS,
};

pub(crate) fn handle(
    app: &mut App,
    route: CommandRoute,
    parts: &[&str],
) -> Result<bool, Box<dyn std::error::Error>> {
    match route {
        CommandRoute::Commands => {
            app.push_system(format!("Commands: {}", COMMAND_SUGGESTIONS.join(", ")));
        }
        CommandRoute::Attach => {
            let path = parts.get(1..).map(|v| v.join(" ")).unwrap_or_default();
            if path.trim().is_empty() {
                app.push_system("Usage: /attach <image_path>".to_string());
            } else {
                let pathbuf = PathBuf::from(path.clone());
                let ext_ok = pathbuf
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(|e| {
                        matches!(
                            e.to_lowercase().as_str(),
                            "png" | "jpg" | "jpeg" | "webp" | "gif"
                        )
                    })
                    .unwrap_or(false);
                if !ext_ok {
                    app.push_system(
                        "Only png/jpg/jpeg/webp/gif attachments are supported.".to_string(),
                    );
                    return Ok(true);
                }
                let bytes = fs::read(&pathbuf)?;
                let encoded = STANDARD.encode(bytes);
                app.pending_images.push(PendingImage {
                    path,
                    base64: encoded,
                });
                app.set_toast(
                    ToastLevel::Info,
                    format!("Attached image ({} pending)", app.pending_images.len()),
                );
            }
        }
        CommandRoute::ClearAttachments => {
            app.pending_images.clear();
            app.set_toast(ToastLevel::Info, "Pending attachments cleared");
        }
        CommandRoute::Url => {
            let url = parts.get(1..).map(|v| v.join(" ")).unwrap_or_default();
            if url.trim().is_empty() {
                app.push_system("Usage: /url <http://host:port>".to_string());
            } else {
                app.url = url;
                app.set_toast(ToastLevel::Success, "Server URL updated for this session");
            }
        }
        _ => return Ok(false),
    }

    Ok(true)
}
