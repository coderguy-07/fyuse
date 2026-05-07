//! Multi-channel bridge — Telegram, Discord, Slack, Matrix, Web Widget.

pub mod router;
pub mod session;
pub mod traits;

#[cfg(feature = "telegram")]
pub mod telegram;

#[cfg(feature = "discord")]
pub mod discord;

#[cfg(feature = "slack")]
pub mod slack;

#[cfg(feature = "matrix")]
pub mod matrix;

pub mod web_widget;

pub use router::{ChannelRouteConfig, ChannelRouter};
pub use session::{Session, SessionManager};
pub use traits::{Channel, ChannelConfig, ChannelType, IncomingMessage, Message};
pub use web_widget::{WebWidgetChannel, WebWidgetConfig, WidgetMessage, WidgetSession};
