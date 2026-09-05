# ---------------------------------------------------------------------------
# handoff.s — the direct-kernel handoff trampoline (report §3.6, §4.4)
#
# The ONLY hand-written assembly in MyBoot. Reached only on the LinuxDirect
# path, AFTER ExitBootServices, with a valid boot_params already built by the
# Rust side. It installs the CPU state the Linux 64-bit boot protocol REQUIRES
# and jumps to the kernel entry point. It never returns.
#
# Authority: Documentation/arch/x86/boot.rst (kernel.org), "64-bit BOOT
# PROTOCOL". At the 64-bit entry point the loader guarantees:
#   * CPU in 64-bit long mode with paging enabled (already true under UEFI).
#   * A GDT loaded whose selectors __BOOT_CS(0x10) and __BOOT_DS(0x18) are
#     4 GiB flat segments: __BOOT_CS execute/read, __BOOT_DS read/write.
#   * CS = __BOOT_CS; DS = ES = SS = __BOOT_DS.
#   * Interrupts disabled.
#   * %rsi holds the base address of struct boot_params (the "zero page").
#
# System V AMD64 in-args (from the Rust `extern "C"` declaration):
#   %rdi = entry      : kernel 64-bit entry point (load_addr + 0x200)
#   %rsi = boot_params: pointer to the populated boot_params
#   %rdx = stack_top  : top of the stack to install before the jump
#
# AT&T syntax is selected via options(att_syntax) on the global_asm! in mod.rs.
# Only format-portable directives are used here, because this file is assembled
# for BOTH the ELF host target (tests) and the PE/COFF x86_64-unknown-uefi
# target (the real loader). ELF-only directives (.type/@function, .size) are
# deliberately omitted — they are metadata and break the COFF assembler.
# ---------------------------------------------------------------------------
    .text
    .globl myboot_jump_to_kernel
myboot_jump_to_kernel:
    cli                              # interrupts MUST be off at entry
    cld                              # forward string ops (ABI-clean state)

    # Preserve the protocol registers before we touch scratch regs.
    #   %rdi = entry, %rsi = boot_params, %rdx = stack_top.
    # We only clobber %rax below, so %rdi/%rsi/%rdx stay intact.

    # --- load our own flat GDT (position-independent) ---
    # Fix up the descriptor's base field to the runtime address of the GDT,
    # so this works regardless of where the firmware relocated our image.
    leaq   myboot_gdt(%rip), %rax
    movq   %rax, myboot_gdt_desc_base(%rip)
    lgdt   myboot_gdt_desc(%rip)

    # --- reload the data segment registers to __BOOT_DS (0x18) ---
    movw   $0x18, %ax
    movw   %ax, %ds
    movw   %ax, %es
    movw   %ax, %ss
    movw   %ax, %fs
    movw   %ax, %gs

    # --- install the kernel stack and clear the frame pointer ---
    movq   %rdx, %rsp
    xorq   %rbp, %rbp

    # boot_params is already in %rsi (in-arg); the protocol requires it there.

    # --- reload CS to __BOOT_CS (0x10) via a far return ---
    # Push (CS=0x10):(RIP=1f) and lretq into our own code segment, then jump
    # to the kernel entry point still held in %rdi.
    leaq   1f(%rip), %rax
    pushq  $0x10                     # __BOOT_CS selector
    pushq  %rax                      # return RIP = label 1
    lretq
1:
    jmp    *%rdi                     # jump to the kernel entry point; no return
    ud2                              # trap if the kernel ever returns (it must not)

# ---------------------------------------------------------------------------
# A minimal 4 GiB flat GDT for the boot protocol. Selector layout:
#   0x00 null | 0x08 unused | 0x10 __BOOT_CS | 0x18 __BOOT_DS
# Descriptor values are the canonical 64-bit flat entries used by the kernel's
# own head_64.S: code = 0x00af9a000000ffff, data = 0x00cf92000000ffff.
# ---------------------------------------------------------------------------
    .data
    .balign 16
myboot_gdt:
    .quad 0x0000000000000000        # 0x00 null descriptor
    .quad 0x0000000000000000        # 0x08 unused
    .quad 0x00af9a000000ffff        # 0x10 __BOOT_CS: 64-bit code, 4G flat, RX
    .quad 0x00cf92000000ffff        # 0x18 __BOOT_DS: data, 4G flat, RW
myboot_gdt_end:

    .balign 8
myboot_gdt_desc:
    .word  myboot_gdt_end - myboot_gdt - 1   # limit (size - 1)
myboot_gdt_desc_base:
    .quad  0                                  # base, fixed up at runtime
