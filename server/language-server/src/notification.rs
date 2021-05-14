use serde::{Deserialize, Serialize};
use lspower::lsp::notification::Notification;
#[derive(Debug, Deserialize, Serialize)]
pub struct CustomNotificationParams {
    title: String,
    message: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct AstPreviewRequestParams {
    pub(crate) path: String
}


impl CustomNotificationParams {
    pub(crate) fn new(title: impl Into<String>, message: impl Into<String>) -> Self {
        CustomNotificationParams {
            title: title.into(),
            message: message.into(),
        }
    }
}

pub enum CustomNotification {}

impl Notification for CustomNotification {
    type Params = CustomNotificationParams;

    const METHOD: &'static str = "tjs-postfix/notification";
}