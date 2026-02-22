#[cfg(test)]
pub mod helpers {
    use crate::model::{Priority, Tag, TodoItem};

    pub fn make_item(file: &str, line: usize, tag: Tag, message: &str) -> TodoItem {
        TodoItem {
            file: file.to_string(),
            line,
            tag,
            message: message.to_string(),
            author: None,
            issue_ref: None,
            priority: Priority::Normal,
            deadline: None,
        }
    }
}
