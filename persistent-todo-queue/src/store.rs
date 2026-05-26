use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use borsh::{BorshDeserialize, BorshSerialize};

use crate::queue::Queue;

pub struct Store {
    path: PathBuf,
}

impl Store {
    pub fn new(path: impl AsRef<Path>) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn load<T: BorshDeserialize>(&self) -> io::Result<Queue<T>> {
        if !self.path.exists() {
            return Ok(Queue::new());
        }
        let bytes = fs::read(&self.path)?;
        if bytes.is_empty() {
            return Ok(Queue::new());
        }
        Queue::<T>::try_from_slice(&bytes)
    }

    pub fn save<T: BorshSerialize>(&self, queue: &Queue<T>) -> io::Result<()> {
        if let Some(parent) = self.path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent)?;
            }
        }
        let bytes = borsh::to_vec(queue)?;
        let tmp = self.path.with_extension("bin.tmp");
        fs::write(&tmp, &bytes)?;
        fs::rename(&tmp, &self.path)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::todo::Todo;
    use tempfile::tempdir;

    fn sample(id: u64, desc: &str) -> Todo {
        Todo {
            id,
            description: desc.to_string(),
            created_at: id * 10,
        }
    }

    #[test]
    fn load_returns_empty_when_file_missing() {
        let dir = tempdir().unwrap();
        let store = Store::new(dir.path().join("missing.bin"));
        let queue: Queue<Todo> = store.load().unwrap();
        assert!(queue.is_empty());
    }

    #[test]
    fn roundtrip_persists_across_load() {
        let dir = tempdir().unwrap();
        let store = Store::new(dir.path().join("todos.bin"));

        let mut queue: Queue<Todo> = Queue::new();
        queue.enqueue(sample(1, "first"));
        queue.enqueue(sample(2, "second"));
        store.save(&queue).unwrap();

        let restored: Queue<Todo> = store.load().unwrap();
        assert_eq!(restored.len(), 2);
        let items: Vec<_> = restored.iter().collect();
        assert_eq!(items[0].id, 1);
        assert_eq!(items[0].description, "first");
        assert_eq!(items[1].id, 2);
        assert_eq!(items[1].description, "second");
    }

    #[test]
    fn save_overwrites_previous_state() {
        let dir = tempdir().unwrap();
        let store = Store::new(dir.path().join("todos.bin"));

        let mut queue: Queue<Todo> = Queue::new();
        queue.enqueue(sample(1, "first"));
        store.save(&queue).unwrap();

        queue.dequeue();
        store.save(&queue).unwrap();

        let restored: Queue<Todo> = store.load().unwrap();
        assert!(restored.is_empty());
    }

    #[test]
    fn fifo_order_preserved_after_reload() {
        let dir = tempdir().unwrap();
        let store = Store::new(dir.path().join("todos.bin"));

        let mut queue: Queue<Todo> = Queue::new();
        for i in 1..=5 {
            queue.enqueue(sample(i, &format!("task {i}")));
        }
        store.save(&queue).unwrap();

        let mut restored: Queue<Todo> = store.load().unwrap();
        for i in 1..=5 {
            let t = restored.dequeue().unwrap();
            assert_eq!(t.id, i);
        }
        assert!(restored.is_empty());
    }

    #[test]
    fn empty_file_loads_as_empty_queue() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("todos.bin");
        fs::write(&path, b"").unwrap();

        let store = Store::new(&path);
        let queue: Queue<Todo> = store.load().unwrap();
        assert!(queue.is_empty());
    }
}
