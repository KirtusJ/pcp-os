# Entry from a Multiboot loader: 32-bit protected mode, paging off, EAX = magic, EBX = information structure.
# This file does the least that must be done before Rust can run: an identity map of the first four gibibytes
# (the kernel uses only the first; the rest is there so that the firmware's tables can be read wherever they lie),
# long mode with no-execute enabled, a stack, and a jump.

.section .multiboot, "a"
.align 4
.long 0x1BADB002
.long 0x00000003
.long -(0x1BADB002 + 0x00000003)

.section .boot.text, "ax"
.code32
.global _start
_start:
    cli
    movl $boot_stack_top, %esp
    movl %eax, %edi
    movl %ebx, %esi

    # Ask before telling: long mode and no-execute must both be offered. Setting either on a processor
    # without it faults before any handler exists, and the machine would reset without a word.
    movl $0x80000000, %eax
    cpuid
    cmpl $0x80000001, %eax
    jb   unsupported
    movl $0x80000001, %eax
    cpuid
    andl $0x20100000, %edx        # bit 29: long mode; bit 20: no-execute
    cmpl $0x20100000, %edx
    jne  unsupported

    movl $boot_pdpt, %eax
    orl  $0x3, %eax
    movl %eax, boot_pml4
    # Four second-level tables, one for each gibibyte, each of 512 pages of 2 MiB.
    xorl %ecx, %ecx
2:  movl %ecx, %eax
    shll $12, %eax
    addl $boot_pd, %eax
    orl  $0x3, %eax
    movl %eax, boot_pdpt(,%ecx,8)
    incl %ecx
    cmpl $4, %ecx
    jne  2b
    xorl %ecx, %ecx
1:  movl %ecx, %eax
    shll $21, %eax
    orl  $0x83, %eax
    movl %eax, boot_pd(,%ecx,8)
    incl %ecx
    cmpl $2048, %ecx
    jne  1b

    movl $boot_pml4, %eax
    movl %eax, %cr3
    movl %cr4, %eax
    orl  $0x20, %eax              # physical address extension
    movl %eax, %cr4
    movl $0xC0000080, %ecx
    rdmsr
    orl  $0x900, %eax             # long mode, and no-execute
    wrmsr
    movl %cr0, %eax
    orl  $0x80010001, %eax        # paging, write protection in supervisor mode, protected mode
    movl %eax, %cr0
    lgdt boot_gdt_ptr
    ljmp $0x08, $long_mode

unsupported:                      # say why on the serial port, and stop
    movl $unsupported_text, %ebx
3:  movb (%ebx), %al
    testb %al, %al
    jz   4f
    movw $0x3F8, %dx
    outb %al, %dx
    incl %ebx
    jmp  3b
4:  hlt
    jmp  4b

.code64
long_mode:
    movw $0x10, %ax
    movw %ax, %ds
    movw %ax, %es
    movw %ax, %ss
    movw %ax, %fs
    movw %ax, %gs
    movq $boot_stack_top, %rsp
    xorq %rbp, %rbp
    call kernel_main
2:  hlt
    jmp  2b

.section .boot.data, "aw"
.align 16
boot_gdt:
    .quad 0
    .quad 0x00AF9A000000FFFF
    .quad 0x00CF92000000FFFF
boot_gdt_ptr:
    .word boot_gdt_ptr - boot_gdt - 1
    .long boot_gdt
unsupported_text:
    .asciz "PCP OS kernel: this processor does not offer 64-bit long mode with no-execute protection. Stopped.\r\n"

.section .bss
.align 4096
boot_pml4: .skip 4096
boot_pdpt: .skip 4096
boot_pd:   .skip 16384
.global boot_stack_guard
boot_stack_guard: .skip 4096     # left unmapped by arch::protect: running off the end of the stack faults instead of overwriting
boot_stack: .skip 65536
boot_stack_top:

# ---- interrupts and exceptions --------------------------------------------------------------------
# Every vector arrives at isr_common with the same frame: the registers, the vector number, an error code
# (zero where the processor supplies none), and what the processor pushed. The handler is Rust.
.section .text
.code64
.macro ISR_NOERR num
isr\num:
    pushq $0
    pushq $\num
    jmp isr_common
.endm
.macro ISR_ERR num
isr\num:
    pushq $\num
    jmp isr_common
.endm
ISR_NOERR 0
ISR_NOERR 1
ISR_NOERR 2
ISR_NOERR 3
ISR_NOERR 4
ISR_NOERR 5
ISR_NOERR 6
ISR_NOERR 7
ISR_ERR 8
ISR_NOERR 9
ISR_ERR 10
ISR_ERR 11
ISR_ERR 12
ISR_ERR 13
ISR_ERR 14
ISR_NOERR 15
ISR_NOERR 16
ISR_ERR 17
ISR_NOERR 18
ISR_NOERR 19
ISR_NOERR 20
ISR_ERR 21
ISR_NOERR 22
ISR_NOERR 23
ISR_NOERR 24
ISR_NOERR 25
ISR_NOERR 26
ISR_NOERR 27
ISR_NOERR 28
ISR_ERR 29
ISR_ERR 30
ISR_NOERR 31
ISR_NOERR 32
ISR_NOERR 33
ISR_NOERR 34
ISR_NOERR 35
ISR_NOERR 36
ISR_NOERR 37
ISR_NOERR 38
ISR_NOERR 39
ISR_NOERR 40
ISR_NOERR 41
ISR_NOERR 42
ISR_NOERR 43
ISR_NOERR 44
ISR_NOERR 45
ISR_NOERR 46
ISR_NOERR 47
ISR_NOERR 128
ISR_NOERR 129
ISR_NOERR 255

# The `syscall` instruction. The processor has put the program's next instruction in RCX and its flags in
# R11, has masked interrupts, and has changed neither the stack nor any other register. There is one
# processor and interrupts are off, so one word is enough to hold the program's stack pointer while the
# kernel's is loaded. The frame built here is the one an interrupt would have built, so the rest of the
# way in and the whole way out (isr_common, iretq) is shared with every other entry.
.global syscall_entry
syscall_entry:
    movq %rsp, syscall_user_rsp(%rip)
    movq syscall_kernel_rsp(%rip), %rsp
    pushq $0x1B
    pushq syscall_user_rsp(%rip)
    pushq %r11
    pushq $0x23
    pushq %rcx
    pushq $0
    pushq $130
    jmp isr_common

# What the processor runs when no program is ready and one is waiting: nothing, until an interrupt.
.global idle_loop
.global idle_loop_end
idle_loop:
    hlt
    jmp idle_loop
idle_loop_end:

.global isr_common
isr_common:
    pushq %rax
    pushq %rcx
    pushq %rdx
    pushq %rbx
    pushq %rbp
    pushq %rsi
    pushq %rdi
    pushq %r8
    pushq %r9
    pushq %r10
    pushq %r11
    pushq %r12
    pushq %r13
    pushq %r14
    pushq %r15
    movq %rsp, %rdi
    cld
    call exception_entry
    popq %r15
    popq %r14
    popq %r13
    popq %r12
    popq %r11
    popq %r10
    popq %r9
    popq %r8
    popq %rdi
    popq %rsi
    popq %rbp
    popq %rbx
    popq %rdx
    popq %rcx
    popq %rax
    addq $16, %rsp
    iretq

# Probes: one instruction each that the memory protections must refuse. When the processor refuses, the
# handler resumes at probe_resume, whose single instruction returns to the caller of the probe.
.global probe_write
probe_write:
    movb %sil, (%rdi)
    ret
.global probe_read
probe_read:
    movzbl (%rdi), %eax
    ret
.global probe_exec
probe_exec:
    call *%rdi
    ret
.global probe_resume
probe_resume:
    ret

.section .rodata
.align 8
.global isr_table
isr_table:
    .quad isr0, isr1, isr2, isr3, isr4, isr5, isr6, isr7, isr8, isr9, isr10, isr11, isr12, isr13, isr14, isr15
    .quad isr16, isr17, isr18, isr19, isr20, isr21, isr22, isr23, isr24, isr25, isr26, isr27, isr28, isr29, isr30, isr31
    .quad isr32, isr33, isr34, isr35, isr36, isr37, isr38, isr39, isr40, isr41, isr42, isr43, isr44, isr45, isr46, isr47
    .quad isr128, isr129, isr255

.section .data
.align 8
.global syscall_kernel_rsp
syscall_kernel_rsp: .quad 0
syscall_user_rsp: .quad 0

# ---- programs for user mode --------------------------------------------------------------------------
# Each program below is a few instructions the kernel copies into a page of a new address space and runs
# in ring 3. They are data to the kernel: it never executes them itself. Each is position-independent.
# A system call is `int $0x80` with the call number in RAX and arguments in RDI and RSI; the answer is in
# RAX. Calls: 0 exit(code), 1 write(address, length), 2 yield, 3 pid, 4 ticks.
.macro USER_PROGRAM name
.balign 16
.global user_\name\()_start
user_\name\()_start:
.endm
.macro USER_END name
.global user_\name\()_end
user_\name\()_end:
.endm
.macro SYS_EXIT
    xorl %eax, %eax
    int $0x80
9:  jmp 9b
.endm

.section .rodata
USER_PROGRAM hello                 # writes one line and exits 0
    leaq 1f(%rip), %rdi
    movq $(2f - 1f), %rsi
    movl $1, %eax
    int $0x80
    xorl %edi, %edi
    SYS_EXIT
1:  .ascii "hello from user mode"
2:
USER_END hello

USER_PROGRAM polite                # writes three lines, yielding after each, and exits 0
    movl $3, %ebx
1:  leaq 3f(%rip), %rdi
    movq $(4f - 3f), %rsi
    movl $1, %eax
    int $0x80
    movl $2, %eax
    int $0x80
    decl %ebx
    jnz 1b
    xorl %edi, %edi
    SYS_EXIT
3:  .ascii "a line, then a yield"
4:
USER_END polite

USER_PROGRAM spinner               # never yields: stores its pid in its data page, computes for several timer periods,
    movl $3, %eax                  # and exits 0 only if the page still holds its own pid
    int $0x80
    movq %rax, %r12
    movabsq $0x8000010000, %r13
    movq %r12, (%r13)
    movl $4, %eax
    int $0x80
    movq %rax, %r14                # ticks at the start
1:  movl $2000000, %ecx
2:  decl %ecx
    jnz 2b
    cmpq %r12, (%r13)
    jne 3f
    movl $4, %eax
    int $0x80
    subq %r14, %rax
    cmpq $12, %rax                 # until twelve timer periods have passed
    jb 1b
    leaq 4f(%rip), %rdi
    movq $(5f - 4f), %rsi
    movl $1, %eax
    int $0x80
    xorl %edi, %edi
    SYS_EXIT
3:  movl $1, %edi
    SYS_EXIT
4:  .ascii "my memory is my own"
5:
USER_END spinner

USER_PROGRAM read_kernel           # reads the kernel's code
    movq __text_start, %rax
    movl $99, %edi
    SYS_EXIT
USER_END read_kernel

USER_PROGRAM write_kernel          # writes into the kernel's memory
    movabsq $__text_start, %rax
    movq $0, (%rax)
    movl $99, %edi
    SYS_EXIT
USER_END write_kernel

USER_PROGRAM privileged            # an instruction reserved to the kernel
    cli
    hlt
    movl $99, %edi
    SYS_EXIT
USER_END privileged

USER_PROGRAM port                  # writes to a device port directly
    movw $0x3F8, %dx
    movb $0x21, %al
    outb %al, %dx
    movl $99, %edi
    SYS_EXIT
USER_END port

USER_PROGRAM exec_data             # jumps into its own writable data
    movabsq $0x8000010000, %rax
    movb $0xC3, (%rax)
    call *%rax
    movl $99, %edi
    SYS_EXIT
USER_END exec_data

USER_PROGRAM overflow              # recurses until its stack is exhausted
1:  call 1b
USER_END overflow

USER_PROGRAM bad_calls             # hands the kernel a kernel address, an unmapped address and an undefined call number
    movq $__text_start, %rdi
    movq $16, %rsi
    movl $1, %eax
    int $0x80
    cmpq $-14, %rax
    jne 1f
    movabsq $0x8000900000, %rdi
    movq $16, %rsi
    movl $1, %eax
    int $0x80
    cmpq $-14, %rax
    jne 1f
    movq $-1, %rsi                 # a length no buffer has
    leaq 1f(%rip), %rdi
    movl $1, %eax
    int $0x80
    cmpq $-22, %rax
    jne 1f
    movl $999, %eax
    int $0x80
    cmpq $-38, %rax
    jne 1f
    xorl %edi, %edi
    SYS_EXIT
1:  movl $1, %edi
    SYS_EXIT
USER_END bad_calls

USER_PROGRAM mute                  # is started without the right to write: exits 0 only if the write is refused
    leaq 1f(%rip), %rdi
    movq $4, %rsi
    movl $1, %eax
    int $0x80
    cmpq $-1, %rax
    jne 2f
    xorl %edi, %edi
    SYS_EXIT
2:  movl $1, %edi
    SYS_EXIT
1:  .ascii "mute"
USER_END mute

USER_PROGRAM hog                   # never yields and never ends
1:  jmp 1b
USER_END hog

# The three programs below are for the self-tests of sleeping, waiting and the stop key. They make their calls
# with the `syscall` instruction and Linux's numbers, as a loaded program does; the self-test puts each behind
# an ELF header and hands it to the loader.
USER_PROGRAM sleeper               # sleeps 50 ms and exits with what the sleep answered (0)
    subq $16, %rsp
    movq $0, (%rsp)
    movq $50000000, 8(%rsp)
    movq %rsp, %rdi
    xorl %esi, %esi
    movl $35, %eax                 # nanosleep
    syscall
    movl %eax, %edi
    movl $60, %eax                 # exit
    syscall
9:  jmp 9b
USER_END sleeper

USER_PROGRAM sleeper_long          # asks for the longest sleep the kernel accepts; exits 7 if it is ever woken
    subq $16, %rsp
    movabsq $0x100000000, %rax
    movq %rax, (%rsp)
    movq $999999999, 8(%rsp)
    movq %rsp, %rdi
    xorl %esi, %esi
    movl $35, %eax
    syscall
    movl $7, %edi
    movl $60, %eax
    syscall
9:  jmp 9b
USER_END sleeper_long

USER_PROGRAM key_reader            # checks Shift down/up then A, or Ctrl then Alt while waiting to be stopped
    leaq 1f(%rip), %rdi
    xorl %esi, %esi
    movl $2, %eax                  # open, for reading
    syscall
    testq %rax, %rax
    js 2f
    movq %rax, %rbx
    leaq 5f(%rip), %r12            # default test expects Shift down/up then A
    cmpq $2, (%rsp)                # the stop test supplies a second argument
    jne 4f
    leaq 6f(%rip), %r12
4:  subq $16, %rsp
3:  movq %rbx, %rdi
    movq %rsp, %rsi
    movl $2, %edx
    xorl %eax, %eax                # read: one event of two bytes
    syscall
    cmpq $2, %rax
    jne 2f
    movzwl (%rsp), %eax
    cmpw (%r12), %ax               # check code and release/extended flags together
    jne 2f
    addq $2, %r12
    cmpl $0x001E, %eax
    jne 3b
    xorl %edi, %edi
    movl $60, %eax
    syscall
2:  movl $1, %edi
    movl $60, %eax
    syscall
9:  jmp 9b
1:  .asciz "/dev/kbd"
5:  .word 0x002A, 0x012A, 0x001E   # Shift down, Shift up, A down
6:  .word 0x001D, 0x0038, 0xFFFF   # Ctrl, Alt; any delivered third key is an error
USER_END key_reader
