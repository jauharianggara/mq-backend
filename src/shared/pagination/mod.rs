//! Cursor pagination (feed/queue/inbox — konvensi kontrak M1.2 Bagian III).
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct CursorPage<T> {
    pub items: Vec<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
    pub has_more: bool,
}

impl<T> CursorPage<T> {
    /// Potong `items` ke `limit`, set `next_cursor` = cursor item terakhir yang dilewati.
    pub fn from_iter(mut items: Vec<T>, limit: usize, cursor_of: impl Fn(&T) -> String) -> Self {
        let has_more = items.len() > limit;
        if has_more {
            items.truncate(limit);
        }
        let next_cursor = if has_more {
            items.last().map(&cursor_of)
        } else {
            None
        };
        Self { items, next_cursor, has_more }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pagination_memotong_dan_cursor() {
        let data = vec!["a", "b", "c", "d"];
        let page = CursorPage::from_iter(data, 2, |s| s.to_string());
        assert_eq!(page.items, vec!["a", "b"]);
        assert_eq!(page.next_cursor.as_deref(), Some("b"));
        assert!(page.has_more);
    }

    #[test]
    fn pagination_habis() {
        let data = vec!["a"];
        let page = CursorPage::from_iter(data, 10, |s| s.to_string());
        assert_eq!(page.items, vec!["a"]);
        assert!(page.next_cursor.is_none());
        assert!(!page.has_more);
    }

    #[test]
    fn pagination_boundary_persis_limit() {
        let data = vec!["a", "b"];
        let page = CursorPage::from_iter(data, 2, |s| s.to_string());
        assert_eq!(page.items.len(), 2);
        assert!(!page.has_more, "data == limit => tidak ada halaman berikutnya");
    }
}
