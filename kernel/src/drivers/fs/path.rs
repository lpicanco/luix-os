const PATH_SEPARATOR: char = '/';

pub struct Path<'a>(&'a str);

impl<'a> Path<'a> {
    pub fn new(path: &'a str) -> Option<Self> {
        if !path.starts_with(PATH_SEPARATOR) {
            return None;
        }

        Some(Self(path))
    }

    pub fn iter(&self) -> impl Iterator<Item = &'a str> {
        let parts = self.0.split(PATH_SEPARATOR);
        parts.filter(|&e| !e.is_empty())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test_case]
    fn test_path_iter() {
        let path = Path::new("/a/b/c.file").unwrap();
        let mut iter = path.iter();
        assert_eq!(iter.next(), Some("a"));
        assert_eq!(iter.next(), Some("b"));
        assert_eq!(iter.next(), Some("c.file"));
        assert_eq!(iter.next(), None);

        let path = Path::new("/c.file").unwrap();
        let mut iter = path.iter();
        assert_eq!(iter.next(), Some("c.file"));

        let path = Path::new("/").unwrap();
        assert_eq!(path.iter().next(), None);
    }
}
