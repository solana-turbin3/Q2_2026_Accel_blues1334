use std::collections::VecDeque;

use borsh::{BorshDeserialize, BorshSerialize};

#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub struct Queue<T> {
    items: VecDeque<T>,
}

impl<T> Queue<T> {
    pub fn new() -> Self {
        Self {
            items: VecDeque::new(),
        }
    }

    pub fn enqueue(&mut self, item: T) {
        self.items.push_back(item);
    }

    pub fn dequeue(&mut self) -> Option<T> {
        self.items.pop_front()
    }

    pub fn peek(&self) -> Option<&T> {
        self.items.front()
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.items.iter()
    }
}

impl<T> Default for Queue<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fifo_order() {
        let mut q: Queue<i32> = Queue::new();
        q.enqueue(1);
        q.enqueue(2);
        q.enqueue(3);

        assert_eq!(q.len(), 3);
        assert_eq!(q.peek(), Some(&1));
        assert_eq!(q.dequeue(), Some(1));
        assert_eq!(q.dequeue(), Some(2));
        assert_eq!(q.dequeue(), Some(3));
        assert_eq!(q.dequeue(), None);
        assert!(q.is_empty());
    }

    #[test]
    fn empty_queue() {
        let mut q: Queue<String> = Queue::new();
        assert!(q.is_empty());
        assert_eq!(q.len(), 0);
        assert!(q.peek().is_none());
        assert!(q.dequeue().is_none());
    }

    #[test]
    fn generic_over_t() {
        let _: Queue<u8> = Queue::new();
        let _: Queue<(u64, String)> = Queue::new();
        let _: Queue<Vec<bool>> = Queue::new();
    }

    #[test]
    fn peek_does_not_consume() {
        let mut q: Queue<i32> = Queue::new();
        q.enqueue(42);
        assert_eq!(q.peek(), Some(&42));
        assert_eq!(q.peek(), Some(&42));
        assert_eq!(q.len(), 1);
    }
}
