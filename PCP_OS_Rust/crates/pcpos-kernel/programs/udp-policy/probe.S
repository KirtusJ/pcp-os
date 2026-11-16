/* The same freestanding ELF runs in raw, restricted and invalid-policy arms. */
.intel_syntax noprefix
.text
.global _start
_start:
    sub rsp, 1536
    mov eax, 2
    lea rdi, [rip+ethernet]
    mov esi, 2050
    syscall
    cmp rax, -13
    je denied
    test rax, rax
    js fail
    mov r12, rax
    lea r13, [rip+rejected]
    mov r14d, REJECT_COUNT
deny_loop:
    mov rsi, r13
    call send
    cmp rax, 60
    je raw_pass
    cmp rax, -13
    jne fail
    add r13, 60
    dec r14d
    jnz deny_loop
    /* The peer queues a matching datagram before our first permitted send. */
    mov eax, 1
    mov edi, 1
    lea rsi, [rip+ready]
    mov edx, 14
    syscall
    mov eax, 35
    lea rdi, [rip+before_send]
    xor esi, esi
    syscall
    test rax, rax
    jne fail
    lea rsi, [rip+permitted]
    call send
    cmp rax, 60
    jne fail
    mov r15d, 1000000
await_reply:
    call read_frame
    cmp rax, -11
    jne got
    dec r15d
    jnz await_reply
    jmp fail
got:
    cmp rax, 60
    jne fail
    mov rax, qword ptr [rip+identity]
    cmp qword ptr [rsp+42], rax
    jne fail
    /* Give the peer time to submit a second copy; it has no remaining credit. */
    mov eax, 35
    lea rdi, [rip+interval]
    xor esi, esi
    syscall
    test rax, rax
    jne fail
    call read_frame
    cmp rax, -11
    jne fail
    lea rsi, [rip+filtered]
    mov edx, 17
    jmp report
raw_pass:
    cmp r14d, REJECT_COUNT
    jne fail
    lea rsi, [rip+raw]
    mov edx, 15
    jmp report
denied:
    lea rsi, [rip+no_grant]
    mov edx, 17
report:
    mov eax, 1
    mov edi, 1
    syscall
    mov eax, 60
    xor edi, edi
    syscall
fail:
    mov eax, 60
    mov edi, 42
    syscall
send:
    mov eax, 1
    mov rdi, r12
    mov edx, 60
    syscall
    ret
read_frame:
    xor eax, eax
    mov rdi, r12
    lea rsi, [rsp+8]
    mov edx, 1536
    syscall
    ret
ethernet: .asciz "/dev/ethernet"
identity: .ascii "VALIDUDP"
ready: .ascii "UDP PRE READY\n"
raw: .ascii "UDP RAW PASSED\n"
filtered: .ascii "UDP FILTERED OK!\n"
no_grant: .ascii "UDP GRANT DENIED\n"
.balign 8
interval: .quad 0, 300000000
before_send: .quad 2, 0
rejected: .incbin "rejected.bin"
permitted: .incbin "permitted.bin"
