//! Named writable files. An open descriptor owns its file independently of its name.
//! The namespace and all open files share a single byte budget, including unlinked files.
#[cfg(test)]
extern crate alloc;

use alloc::{rc::Rc, string::String, vec::Vec};
use core::cell::{Cell, Ref, RefCell};

const LIMIT: usize = 16 << 20;

#[derive(Default)]
pub struct Files {
    names: Vec<(String, File)>,
    used: Rc<Cell<usize>>,
}

#[derive(Clone, Debug)]
pub struct File(Rc<Contents>);

#[derive(Debug)]
struct Contents {
    bytes: RefCell<Vec<u8>>,
    used: Rc<Cell<usize>>,
}

impl Drop for Contents {
    fn drop(&mut self) {
        self.used.set(self.used.get() - self.bytes.get_mut().len());
    }
}

impl Files {
    pub fn len(&self) -> usize {
        self.names.len()
    }

    pub fn iter(&self) -> impl Iterator<Item = &(String, File)> {
        self.names.iter()
    }

    pub fn named(&self, name: &str) -> Option<&File> {
        self.names.iter().find(|(n, _)| n == name).map(|(_, f)| f)
    }

    pub fn linked(&self, file: &File) -> bool {
        self.names.iter().any(|(_, f)| Rc::ptr_eq(&f.0, &file.0))
    }

    /// An unused name and either an empty buffer or bytes from a validated 64 KiB snapshot.
    pub fn insert(&mut self, name: String, bytes: Vec<u8>) -> File {
        self.used.set(self.used.get() + bytes.len());
        let file = File(Rc::new(Contents { bytes: RefCell::new(bytes), used: self.used.clone() }));
        self.names.push((name, file.clone()));
        file
    }

    pub fn unlink(&mut self, name: &str) -> bool {
        let Some(i) = self.names.iter().position(|(n, _)| n == name) else { return false };
        self.names.remove(i);
        true
    }

    pub fn rename(&mut self, from: &str, to: String) -> bool {
        if from == to {
            return self.named(from).is_some();
        }
        let Some(i) = self.names.iter().position(|(name, _)| name == from) else { return false };
        // Remove a replaced destination after renaming so its removal cannot shift the source index.
        let destination = self.names.iter().position(|(name, _)| *name == to);
        self.names[i].0 = to;
        if let Some(index) = destination {
            self.names.remove(index);
        }
        true
    }
}

impl File {
    pub fn len(&self) -> usize {
        self.0.bytes.borrow().len()
    }

    pub fn bytes(&self) -> Ref<'_, [u8]> {
        Ref::map(self.0.bytes.borrow(), Vec::as_slice)
    }

    pub fn truncate(&self) {
        let mut bytes = self.0.bytes.borrow_mut();
        self.0.used.set(self.0.used.get() - bytes.len());
        *bytes = Vec::new();
    }

    /// Return the new descriptor offset. Charge only growth to the shared budget.
    pub fn write(&self, offset: usize, input: &[u8]) -> Option<usize> {
        if input.is_empty() {
            return Some(offset);
        }
        let end = offset.checked_add(input.len())?;
        let mut bytes = self.0.bytes.borrow_mut();
        let growth = end.saturating_sub(bytes.len());
        let used = self.0.used.get();
        if growth > LIMIT - used {
            return None;
        }
        if growth != 0 {
            bytes.resize(end, 0);
            self.0.used.set(used + growth);
        }
        bytes[offset..end].copy_from_slice(input);
        Some(end)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unlinked_bytes_count_until_the_last_handle_drops() {
        let mut files = Files::default();
        let original = files.insert("note".into(), Vec::new());
        let second = original.clone();
        assert!(files.unlink("note"));
        let fresh = files.insert("note".into(), Vec::new());
        assert_eq!(original.write(LIMIT - 1, b"x"), Some(LIMIT));
        assert_eq!(fresh.write(0, b"y"), None);
        drop(original);
        assert_eq!(fresh.write(0, b"y"), None);
        drop(second);
        assert_eq!(fresh.write(0, b"y"), Some(1));
        assert_eq!(files.len(), 1);
    }

    #[test]
    fn truncation_releases_loaded_bytes_and_failed_growth_keeps_the_file() {
        let mut files = Files::default();
        let old = files.insert("saved".into(), b"old".to_vec());
        let new = files.insert("draft".into(), b"new".to_vec());
        assert!(files.rename("draft", "saved".into()));
        assert!(!files.linked(&old));
        assert!(files.linked(&new));
        assert_eq!(old.write(LIMIT - 1, b"x"), None);
        assert_eq!(&*old.bytes(), b"old");
        new.truncate();
        assert_eq!(old.write(LIMIT - 1, b"x"), Some(LIMIT));
        assert_eq!(old.write(usize::MAX, b"x"), None);
        assert_eq!(old.write(usize::MAX, b""), Some(usize::MAX));
        assert_eq!(old.len(), LIMIT);
    }
}
