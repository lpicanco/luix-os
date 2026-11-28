#[derive(Debug)]
pub struct RingBuffer<T, const N: usize> {
    buffer: [Option<T>; N],
    head: usize,
    tail: usize,
}

impl<T, const N: usize> RingBuffer<T, N> {
    pub const fn new() -> Self {
        Self {
            buffer: [const { None }; N],
            head: 0,
            tail: 0,
        }
    }

    pub fn push(&mut self, value: T) {
        if N < 2 {
            return;
        }
        if (self.head + 1) % N == self.tail {
            self.buffer[self.tail].take();
            self.tail = (self.tail + 1) % N;
        }
        self.buffer[self.head] = Some(value);

        self.head = (self.head + 1) % N;
    }

    pub fn pop(&mut self) -> Option<T> {
        if N < 2 {
            return None;
        }
        if self.head == self.tail {
            return None;
        }

        let value = self.buffer[self.tail].take();

        self.tail = (self.tail + 1) % N;
        value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_push_pop() {
        let mut ring_buffer = RingBuffer::<u32, 3>::new();

        ring_buffer.push(1);
        assert_eq!(ring_buffer.pop(), Some(1));
        ring_buffer.push(2);
        assert_eq!(ring_buffer.pop(), Some(2));
        ring_buffer.push(3);
        assert_eq!(ring_buffer.pop(), Some(3));
        ring_buffer.push(4);
        ring_buffer.push(5);
        assert_eq!(ring_buffer.pop(), Some(4));
        assert_eq!(ring_buffer.pop(), Some(5));
        assert_eq!(ring_buffer.pop(), None);
    }

    #[test]
    fn test_push_cycle() {
        let mut ring_buffer = RingBuffer::<u32, 3>::new();

        ring_buffer.push(1);
        ring_buffer.push(2);
        assert_eq!(ring_buffer.pop(), Some(1));
        assert_eq!(ring_buffer.pop(), Some(2));
        assert_eq!(ring_buffer.pop(), None);
        ring_buffer.push(3);
        ring_buffer.push(4);
        ring_buffer.push(5);
        ring_buffer.push(6);
        assert_eq!(ring_buffer.pop(), Some(5));
        assert_eq!(ring_buffer.pop(), Some(6));
        assert_eq!(ring_buffer.pop(), None);
    }
}
