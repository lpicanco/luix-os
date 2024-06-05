pub const SPAWN: usize = 0x1;
pub const EXIT: usize = 0x2;
// TODO: Remove this. This syscall is for testing purposes only.
pub const PRINT_LINE: usize = 0x404;

#[macro_export]
macro_rules! make_syscall {
    ($n:expr) => {
        $crate::arch::syscall::syscall0($n as usize)
    };
    ($n:expr, $a1:expr) => {
        $crate::arch::syscall::syscall1($n as usize, $a1 as usize)
    };
    ($n:expr, $a1:expr, $a2:expr) => {
        $crate::arch::syscall::syscall2($n as usize, $a1 as usize, $a2 as usize)
    };
}

#[inline(always)]
pub fn spawn(path: &str) {
    unsafe {
        make_syscall!(SPAWN, path.as_ptr() as usize, path.len());
    }
}

#[inline(always)]
pub fn exit() {
    // TODO: add enum for exit code
    unsafe {
        make_syscall!(EXIT);
    }
}

#[inline(always)]
pub fn println(s: &str) {
    unsafe {
        make_syscall!(PRINT_LINE, s.as_ptr() as usize, s.len());
    }
}
