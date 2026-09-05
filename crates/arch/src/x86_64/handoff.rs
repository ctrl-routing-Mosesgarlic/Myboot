//! Rust side of the direct-kernel handoff (report §3.6 LinuxDirect, §4.4).
//! Prepares the boot parameters, then calls the assembly trampoline in
//! handoff.s. Used ONLY on the direct-load path; chainloading does not touch this.

extern "C" {
    /// Defined in handoff.s. Never returns.
    ///   entry     : kernel entry-point physical address
    ///   boot_params: pointer to the protocol's boot-parameter block
    ///   stack_top : top of the stack to install before the jump
    fn myboot_jump_to_kernel(entry: u64, boot_params: u64, stack_top: u64) -> !;
}

/// Perform the final jump. MUST be called only after ExitBootServices has
/// succeeded and interrupts are disabled. Never returns on success.
///
/// # Safety
/// `entry`, `boot_params` and `stack_top` must be valid per the boot protocol,
/// boot services must already be exited, and no firmware call may follow.
pub unsafe fn jump_to_kernel(entry: u64, boot_params: u64, stack_top: u64) -> ! {
    myboot_jump_to_kernel(entry, boot_params, stack_top)
}
