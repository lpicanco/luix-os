use core::{fmt, hint};

use crate::bits::Bits;
use crate::drivers::nvme::command::NvmeCommand;
use crate::memory::address::PhysicalAddress;
use crate::memory::allocator::dma_allocator::Dma;

/// A group of queues that are used to submit and complete commands.
struct Submission;
struct Completion;
pub struct QueueGroup {
    submission: Queue<Submission>,
    completion: Queue<Completion>,
    pub queue_size: usize,
    pub queue_group_id: u16,
}

impl QueueGroup {
    pub fn new(queue_group_id: u16, queue_size: usize, base_address: usize) -> Self {
        Self {
            submission: Queue::new(queue_group_id, queue_size, base_address),
            completion: Queue::new(queue_group_id, queue_size, base_address),
            queue_size,
            queue_group_id,
        }
    }

    pub fn submit_command(&mut self, command: NvmeCommand) {
        self.submission.submit_command(command);
        self.completion.wait_for_completion();
    }

    pub fn submission_queue_addr(&self) -> PhysicalAddress {
        self.submission.commands.addr()
    }

    pub fn completion_queue_addr(&self) -> PhysicalAddress {
        self.completion.commands.addr()
    }
}

trait QueueType {
    type Type;
    const DOORBELL_MULTIPLIER: usize;
}

impl QueueType for Submission {
    type Type = NvmeCommand;
    const DOORBELL_MULTIPLIER: usize = 0;
}

impl QueueType for Completion {
    type Type = NvmeCompletion;
    const DOORBELL_MULTIPLIER: usize = 1;
}

struct Queue<T: QueueType> {
    tail: usize,
    commands: Dma<[T::Type]>,
    door_bell_address: usize,
    phase: bool,
}

impl<T: QueueType> Queue<T> {
    pub fn new(queue_id: u16, queue_size: usize, base_address: usize) -> Self {
        let doorbell_offset = (((queue_id as usize) * 2) + T::DOORBELL_MULTIPLIER) * (4 << 0);
        let door_bell_address = base_address + doorbell_offset;

        Self {
            tail: 0,
            commands: Dma::new_zeroed_slice(queue_size).assume_init(),
            door_bell_address,
            phase: true,
        }
    }

    fn ring_doorbell(&mut self) {
        let door_bell = self.door_bell_address as *mut u32;
        unsafe { door_bell.write_volatile(self.tail as u32) };
    }
}

impl Queue<Submission> {
    pub fn submit_command(&mut self, mut command: NvmeCommand) {
        // TODO: Handle queue full condition
        command.command_id = self.tail as u16;
        self.commands[self.tail] = command;
        self.tail = (self.tail + 1) % self.commands.len();
        self.ring_doorbell();
    }
}

impl Queue<Completion> {
    pub fn wait_for_completion(&mut self) {
        while self.commands[self.tail].phase() != self.phase {
            hint::spin_loop();
        }

        self.tail = (self.tail + 1) % self.commands.len();
        if self.tail == 0 {
            self.phase = !self.phase;
        }

        self.ring_doorbell();
    }
}

#[repr(C)]
pub struct NvmeCompletion {
    pub dw0: u32,          // Command specific result
    pub dw1: u32,          // Command specific result
    pub sq_head: u16,      // Submission Queue Head Pointer.
    pub sq_id: u16,        // Submission Queue Identifier.
    pub command_id: u16,   // Command ID of the command that was completed.
    pub phase_status: u16, // Phase and Reason why the command failed, if it did.
}

impl NvmeCompletion {
    pub fn status(&self) -> u16 {
        self.phase_status.get_bits(1..16)
    }

    pub fn phase(&self) -> bool {
        self.phase_status.get_bit(0)
    }

    pub fn is_success(&self) -> bool {
        self.status() == 0
    }

    pub fn status_code(&self) -> u16 {
        self.phase_status.get_bits(1..10)
    }
}

impl fmt::Debug for NvmeCompletion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NvmeCompletion")
            .field("sq_head", &self.sq_head)
            .field("sq_id", &self.sq_id)
            .field("command_id", &self.command_id)
            .field("phase_status", &self.phase_status)
            .field("status", &self.status())
            .field("status_code", &self.status_code())
            .field("phase", &self.phase())
            .finish()
    }
}
