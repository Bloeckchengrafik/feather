use libcraft_text::TextComponent;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PlayerChatEvent {
    pub message: String,
    pub component: TextComponent,
}
