#![allow(unused)]

use std::sync::Mutex;

use crate::common::*;

const MIN_SIZE: u32 = 2;
const MAX_SIZE: u32 = 2 ^ 16;

struct QueueState {
    size: u32,
    head: u16,
    tail: u16,
}
impl QueueState {
    fn new(size: u32, head: u16, tail: u16) -> Self {
        assert!(size >= MIN_SIZE && size <= MAX_SIZE);
        Self { size, head, tail }
    }
    fn is_empty(&self) -> bool {
        // 4.1.1 Empty Queue
        //
        // The queue is Empty when the Head entry pointer equals the Tail entry
        // pointer.
        self.head == self.tail
    }
    fn is_full(&self) -> bool {
        // 4.1.2 Full Queue
        //
        // The queue is Full when the Head equals one more than the Tail.  The
        // number of entries in a queue when full is one less than the queue
        // size.
        (self.head > 0 && self.tail == (self.head - 1))
            || (self.head == 0 && self.tail == (self.size - 1) as u16)
    }
    fn push_tail(&mut self) -> Option<u16> {
        if self.is_full() {
            None
        } else {
            let result = Some(self.tail);

            let next = self.tail as u32 + 1;
            if next == self.size {
                self.tail = 0;
            } else {
                self.tail = next as u16;
            }

            result
        }
    }
    fn pop_head(&mut self) -> Option<u16> {
        if self.is_empty() {
            None
        } else {
            let result = Some(self.head);

            let next = self.head as u32 + 1;
            if next == self.size {
                self.head = 0;
            } else {
                self.head = next as u16;
            }

            result
        }
    }
    fn produce_tail_to(&mut self, idx: u16) -> Result<(), &'static str> {
        if idx as u32 >= self.size {
            return Err("invalid index");
        }
        todo!()
    }
}

pub struct SubQueue {
    state: Mutex<QueueState>,
    base: GuestAddr,
}
impl SubQueue {
    pub fn new(size: u32) -> Self {
        Self {
            state: Mutex::new(QueueState::new(size, 0, 0)),
            // XXX: check addr
            base: GuestAddr(0),
        }
    }
    pub fn notify_tail(&self, idx: u16) {}
}

pub struct CompQueue {
    state: Mutex<QueueState>,
    base: GuestAddr,
}
impl CompQueue {
    pub fn new(size: u32) -> Self {
        Self {
            state: Mutex::new(QueueState::new(size, 0, 0)),
            // XXX: check addr
            base: GuestAddr(0),
        }
    }
    pub fn notify_head(&self, idx: u16) {}
}
