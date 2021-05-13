// use serde::{Deserialize, Serialize};
// use tower_lsp::lsp_types::notification::Notification;
// #[derive(Debug, Deserialize, Serialize)]
// pub struct CustomNotificationParams {
//     title: String,
//     message: String,
// }

// impl CustomNotificationParams {
//     pub(crate) fn new(title: impl Into<String>, message: impl Into<String>) -> Self {
//         CustomNotificationParams {
//             title: title.into(),
//             message: message.into(),
//         }
//     }
// }

// pub enum CustomNotification {}

// impl Notification for CustomNotification {
//     type Params = CustomNotificationParams;

//     const METHOD: &'static str = "tjs-postfix/notification";
// }