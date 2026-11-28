use core::sync::atomic::{fence, Ordering};
use core::{cell::UnsafeCell, hint};

use crate::legacy_drivers::nvme::command::NvmeCommand;
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

    pub fn submit_command(&mut self, command: NvmeCommand) -> Result<(), u16> {
        self.submission.submit_command(command);
        self.completion.wait_for_completion()
    }

    pub fn submission_queue_addr(&self) -> PhysicalAddress {
        self.submission.commands.addr()
    }

    pub fn completion_queue_addr(&self) -> PhysicalAddress {
        self.completion.commands.addr()
    }
}

trait QueueType {
    type Type: crate::memory::allocator::dma_allocator::Zeroable;
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
    commands: Dma<[UnsafeCell<T::Type>]>,
    door_bell_address: usize,
    phase: bool,
}

impl<T: QueueType> Queue<T> {
    pub fn new(queue_id: u16, queue_size: usize, base_address: usize) -> Self {
        let doorbell_offset = (((queue_id as usize) * 2) + T::DOORBELL_MULTIPLIER) * (4 << 0);
        let door_bell_address = base_address + doorbell_offset;

        Self {
            tail: 0,
            commands: Dma::zeroed_slice(queue_size),
            door_bell_address,
            phase: true,
        }
    }

    fn ring_doorbell(&mut self) {
        let door_bell = self.door_bell_address as *mut u32;
        // SAFETY: the queue owns this aligned, mapped doorbell. Its mutex serializes writes.
        unsafe { door_bell.write_volatile(self.tail as u32) };
    }
}

impl Queue<Submission> {
    pub fn submit_command(&mut self, mut command: NvmeCommand) {
        // TODO: Handle queue full condition
        command.command_id = self.tail as u16;
        // SAFETY: blocking submission has completed the previous request before reusing this slot.
        unsafe {
            self.commands[self.tail].get().write(command);
        }
        fence(Ordering::Release);
        self.tail = (self.tail + 1) % self.commands.len();
        self.ring_doorbell();
    }
}

impl Queue<Completion> {
    pub fn wait_for_completion(&mut self) -> Result<(), u16> {
        // SAFETY: tail indexes a live completion entry. No reference to device modified data is formed.
        let status_ptr = unsafe { &raw const (*self.commands[self.tail].get()).phase_status };
        let phase_status = loop {
            // The controller publishes the completion by toggling the phase bit.
            // SAFETY: the retained UnsafeCell entry permits device updates. The aligned
            // volatile read observes the scalar status without borrowing the entry.
            let value = unsafe { status_ptr.read_volatile() };
            if (value & 1 != 0) == self.phase {
                break value;
            }
            hint::spin_loop();
        };
        fence(Ordering::Acquire);

        self.tail = (self.tail + 1) % self.commands.len();
        if self.tail == 0 {
            self.phase = !self.phase;
        }

        self.ring_doorbell();
        // SC and SCT together identify success. The phase and status flags do not.
        if (phase_status >> 1) & 0x7ff == 0 {
            Ok(())
        } else {
            Err(phase_status >> 1)
        }
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

// SAFETY: completion records contain only integer scalars.
unsafe impl crate::memory::allocator::dma_allocator::Zeroable for NvmeCompletion {}
