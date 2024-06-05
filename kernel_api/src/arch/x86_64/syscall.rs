use core::arch::asm;

#[inline(always)]
pub unsafe fn syscall0(syscall_number: usize) -> usize {
    let res: usize;
    asm!(
    "int 0x80", in("rax") syscall_number,
    lateout("rax") res
    );
    res
}

#[inline(always)]
pub unsafe fn syscall1(syscall_number: usize, arg1: usize) -> usize {
    let res: usize;
    asm!(
    "int 0x80",
    in("rax") syscall_number,
    in("rdi") arg1,
    lateout("rax") res
    );
    res
}

#[inline(always)]
pub unsafe fn syscall2(syscall_number: usize, arg1: usize, arg2: usize) -> usize {
    let res: usize;
    asm!(
    "int 0x80", in("rax") syscall_number,
    in("rdi") arg1, in("rsi") arg2,
    lateout("rax") res
    );
    res
}
