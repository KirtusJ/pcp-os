/* Save the guest's initial stack for the original resident worker entry point. */
.text
.globl core_entry
.type core_entry, @function
core_entry:
    movq %rsp, %r12
    andq $-16, %rsp
    call read_core
    testl %eax, %eax
    jnz .Lfailed
    movq %r12, %rsp
    jmp _start
.Lfailed:
    movl %eax, %edi
    movl $60, %eax
    syscall
    ud2
.size core_entry, .-core_entry
.section .note.GNU-stack,"",@progbits
