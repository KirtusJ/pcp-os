# PCP OS Specification

**Product:** Parrot Council Protocol (PCP), Version 1.0. **Component:** PCP OS, build 22. **Method carried:** Adversarial Claim Review (ACR) 1.0.
**Creator:** Kirtus Justus. **Date:** 13 November 2026.
**Status of this document:** a design specification for a system in early development. Most of it describes what is to be built. Each requirement says which.

## 0. How to read this

Each requirement is one numbered sentence. Most hold one requirement and one key word; where a sentence holds two joined by "and", the data form lists both key words. The key words MUST, MUST NOT, SHOULD and MAY are used as BCP 14 defines them (RFC 2119, RFC 8174), and bind only in capitals. Each requirement carries a status:

| Status | Meaning |
| --- | --- |
| **BUILT** | Implemented, and checked by a test or self-test in this build. The check is named |
| **PARTIAL** | Part is implemented and checked; the sentence says which part is not |
| **DESIGN** | Not implemented. A statement of intent, and nothing may be claimed from it |

A requirement marked DESIGN is not a property of PCP OS today. `PCP_OS_Requirements.json` is this section list as data.

PCP OS has not been peer reviewed, independently reviewed or evaluated in use. It has been started only in an emulator. Nothing in it is certified, and it is not fit for use where a failure can harm people.

## 1. Two paths

The central design rule. A computer exists to do the person's work. ACR is a method for examining claims. If the method's rules were checked on every operation, the processor would spend its time arguing about work instead of doing it. So the system has two paths that do not touch.

The **work path** is everything a program does: computing, reading, writing, drawing, talking to devices. No rule of ACR is evaluated on it.

The **record path** is the act of offering an event to a register: "this review was commissioned", "this check ran", "this finding was reached". A guard runs here, once per event, and nowhere else.

**P-1.** The kernel MUST NOT evaluate an ACR guard while handling a system call, an interrupt, a memory fault, a context switch or the completion of an input or output operation. *PARTIAL: system calls, the timer interrupt, faults from user mode and the switch between processes exist and evaluate no guard (`user.rs` calls none; the kernel records processes in its register after they have all ended). No input or output completion path exists.*

**P-2.** An ACR guard MUST be evaluated only when an event is offered to a register. *BUILT: the guards are reachable only through `Record::submit`, `Record::open` and `Record::extend` in the core.*

**P-3.** For a writer that has already checked a register, the cost of offering one more event MUST NOT grow with the number of events already in it, beyond reading the file. *BUILT: measured in this build at 0.28 ms per stored event at 100, 500 and 2,000 events, each event written durably; before this build it was 1.25 ms, 5.3 ms and 21 ms. Test: continuing a checked record equals checking the whole, at every split.*

**P-4.** While ACR is not in use, the system MUST do no ACR work beyond establishing that it is not in use. *BUILT for the host program: a hook with no register present reads nothing and writes nothing. DESIGN for the kernel.*

**P-5.** The kernel MUST NOT schedule, delay or reprioritise a task because of the state of a review. *PARTIAL: the scheduler takes the next process in turn and reads nothing else; the kernel holds no review, so the rule has not been tested against one.*

**P-6.** A halt of ACR use MUST be accepted in every state in which anything is in use. *BUILT: state-machine test over every state; host tests for a mismatched label, a register holding a refused event, and 64 open reviews.*

**P-7.** The system MUST start and be usable on a machine with no graphics processor, no network and no accelerator. *DESIGN: today it has no use for any of them.*

**P-8.** The system MUST NOT open a network connection that the person directing it did not cause. *PARTIAL: the LINK state machine starts OFFLINE and the guards refuse a retrieved artifact without its source; there is no network stack to enforce it on.*

## 2. The kernel

A new kernel for x86_64, written in Rust, with one file of assembly for entry and for interrupt stubs.

**K-1.** The kernel MUST contain no code from another kernel and MUST NOT require another operating system to start. *BUILT: its only dependency is the PCP OS core; it is started directly by a Multiboot loader.*

**K-2.** Before enabling a processor feature, the kernel MUST ask the processor whether it offers that feature, and MUST report and stop where a required one is absent. *BUILT: started as two processors without long mode; each printed the reason and halted.*

**K-3.** No memory MUST be both writable and executable. *BUILT: the page tables are read back and the count is zero; a write to code, a write to read-only data and execution of data are each attempted and refused by the processor.*

**K-4.** The kernel MUST map its own image, the files the loader handed it (read-only), the memory it gives to programs (never executable in kernel mode) and the memory of the devices it drives, and nothing else. *BUILT: the page tables are read back and counted by kind (in the test run: 4,371 pages of image, the program files read-only, 239,616 KiB of frame memory, the display's memory once the display is found); reads of the text display, the low mebibyte, memory above the frame memory and where devices are, are attempted and refused; an instruction placed in frame memory is refused. Until build 20 the map was the image alone, because nothing else existed to map.*

**K-5.** The interrupt table, the segment table and the task state segment MUST be read-only once set. *BUILT: a write to each is attempted and refused.*

**K-6.** A double fault, a non-maskable interrupt and a machine check MUST each be delivered on a stack of their own, and every stack MUST have an unmapped page below it. *BUILT: a read of each of the five guard pages is refused (the kernel stack, the three fault stacks and the stack for entries from user mode); the stack-exhaustion demonstration ends in a reported double fault.*

**K-7.** The kernel MUST turn on supervisor execution prevention, supervisor access prevention and user-mode instruction prevention wherever the processor offers them. *BUILT: read back from the control register on a processor with all three and on one with none.*

**K-8.** An exception the kernel did not prepare for MUST stop the kernel with a report, and MUST NOT be continued past. *BUILT: the fault demonstration.*

**K-9.** Arithmetic overflow in the kernel MUST stop the kernel. *BUILT: overflow checks are compiled into the release build.*

**K-10.** Code that the compiler cannot check for memory safety MUST be confined to named modules, as few as the machine requires, and each use MUST state why it is sound. *BUILT: two files. `arch.rs` (the processor and memory) and `dev.rs` (device registers and device memory); every other file, including the loader, the system calls, the process table and the session, refuses it at compile time. Until build 20 this was one file; drivers are the reason for the second.*

**K-11.** The kernel MUST record its own start, verification and stop in a register, and verification MUST end in OPERATIONAL or in SAFE and nothing else. *BUILT: the kernel's register replays with no problem. Since build 22 a kernel whose verification failed starts no program, by itself or from its menu, and says why. Until then SAFE was recorded and shown and a program could still be started: found while adding the comparison of K-12, which is the first way to make verification fail on purpose.*

**K-12.** The kernel MUST measure its running image and compare the measurement with a value it did not compute. *PARTIAL. It measures its code and read-only data. Since build 22 it compares: whoever starts it may name the expected measurement on the loader's command line (`image=`), and a tool computes that value from the image file without running it (`tools/measure.py`); the kernel's own measurement and the tool's agree. Named the right value the kernel verifies, with the comparison as one of its self-tests; named another, verification fails, the kernel is in SAFE and starts no program. This rests on two things done in build 22: the same source now gives the same image whatever folder it is kept in (the build compiles it in one fixed folder; until then the digest followed the folder's name), so a value computed by one person holds for another who builds the same source with the same compiler installed in the same place; and a kernel that fails verification no longer starts programs, which until build 22 it would have. Still missing: the command line is written by whoever starts the machine and is no root of trust; there is no signed image and no measured start, and with no value named the kernel measures and compares with nothing.*

**K-13.** No code supplied by a third party MUST run in kernel mode. *DESIGN.*

**K-14.** The kernel SHOULD be loaded at an address that differs between starts. *DESIGN: it is loaded at a fixed address.*

**K-15.** The kernel MUST have a source of randomness before it issues any identifier or key. *DESIGN.*

Present since build 17: an address space for each program, user mode, system calls through one gate, a scheduler driven by the timer, and a budget after which a process is stopped. Each is checked by a self-test in which a program attempts the thing and the kernel reports what the processor or the kernel itself answered.

Present since build 21:

- **A program format and a loader.** A program is a statically linked, position-independent ELF file for x86_64, handed to the kernel by the loader as a file beside it. The kernel checks it before running it and refuses one that is truncated, for another processor, linked to a fixed address, in need of a dynamic loader, or asking for memory that is writable and executable at once. Its memory comes from a pool of frames and goes back to the pool when the process ends.
- **A system-call interface that existing programs already speak.** The numbers and conventions of Linux on x86_64, for the subset a single-threaded C program needs: files, memory, time, sleeping, a terminal. A program built with an ordinary C library runs unmodified. The interface is borrowed; no code is.
- **Capabilities in front of every device.** A program reaches the display, the keyboard, sound, the clock or the store only through a file under `/dev`, and opening one is a capability check against what the session granted to that process. With nothing granted, a program computes and nothing else: this is tested.
- **Drivers**, each for a device as QEMU presents it: the display adapter (found on the PCI bus, mode set through its own registers, two pages so that a picture is shown only when complete), the PS/2 keyboard, an AC'97 sound device for sampled sound, an OPL2 synthesiser for music with register writes timed by the kernel, a 1 kHz timer.
- **Floating-point and vector state** kept for each process.
- **Blocking and idling**: a process that waits for a key, for time or for room in the sound buffer does not run, and a kernel with nothing to run halts the processor until an interrupt.
- **A session**: a launcher drawn by the kernel that lists the programs it was handed with what each is granted, starts one, and returns when it ends; a view of the kernel's own register; one key combination (Ctrl+Alt+Del) that the kernel keeps for itself and that stops the running program whatever it is doing.

The test of all this together is a program nobody wrote for it: Doom (section 8a).

Not present: storage that survives a stop, a file system, a network stack, a mouse, threads within a process, signals, more than one processor, messages between processes, drivers for real hardware, a graphics processor, start from UEFI. It has not been started on hardware.

## 3. Authority and administration

**A-1.** Authority MUST be held as capabilities, and a capability derived from another MUST carry the same rights or fewer. *BUILT in the core: derivation, narrowing and revocation tests. PARTIAL in the kernel: each process is a holder, and the write call is answered only for a process that holds a capability for the console (self-test: a program started without it is refused). One object and one right; no capability passes between processes.*

**A-2.** There MUST be no identity that holds every right. *DESIGN.*

**A-3.** No combination of roles MUST give a model or a program the right to start ACR use, to administer, or to change the system. *BUILT: exhaustive over every kind of identity, set of roles and edition.*

**A-4.** Administration MUST be a role held by a person and exercised only while the system is in MAINTENANCE. *PARTIAL: the SYSTEM state machine has MAINTENANCE and its transitions, and entering it asks every task to stop. Nothing yet enforces it on a running system.*

**A-5.** Every administrative act MUST be an event in the register before it takes effect. *DESIGN.*

**A-6.** In the Commercial and Enterprise Editions, whoever administers an installation MUST NOT assess under it. *PARTIAL: stated in the edition profile; the host program refuses a two-person standard in the Personal Edition; separation of the two roles is not enforced.*

## 4. What of ACR can be changed, when, and by whom

The balance between the method and the machine is settled by one fact: everything is identified by a digest, and every record says which digests it was made under. Nothing therefore needs to be forbidden to the person who owns the machine. What must hold is that a change is never silent.

| Thing | Who may change it | When | What the record shows |
| --- | --- | --- | --- |
| The native guards | The person, by replacing the kernel image | In MAINTENANCE | A different image measurement in every later record |
| The method release | Nobody in place. Another release is another identity | Between reviews | The release each register is pinned to, and whether the text at hand was verified against it |
| A released module | Nobody in place. A changed copy is another module | Between reviews | Its own digest |
| A module of the person's own | The person | Between reviews | Its identifier and digest |
| The workspace's modules | The person directing the work | Any time; applies to reviews commissioned afterwards | Who selected them and how the host knows |
| A standard | A person, acting as themselves | Any time | It narrows what the method allows and never widens it |
| An open review's method and modules | Nobody | Never while it is open | Fixed at commission |

**M-1.** Every register MUST state the method release it is held to, by digest. *BUILT.*

**M-2.** A review MUST keep the method release and the modules it was commissioned under until it is closed. *BUILT for modules, which are fixed in the commission event. PARTIAL for the release, which is fixed per register.*

**M-3.** Adding or removing a module MUST require no change to any program. *BUILT in the host program, the plugins and the machine form: a module is found by the header of its own Specification, and a test adds one the program has never heard of. The method packet's own Python tools still carry a list.*

**M-4.** A commission MUST name each selected module by identifier, version and the digest of its Specification, and each MUST be a module found in the method text at hand. *BUILT: tests for a wrong digest, a wrong version, an unknown module, Core listed, and a module listed twice.*

**M-5.** A module written for another Core than the one a program implements MUST NOT be selected. *BUILT.*

**M-6.** A participant MUST NOT select a module; selection is the act of the person directing the work or of a policy they set. *BUILT: recorded by the host from a typed command or a terminal; refused from a session.*

**M-7.** A licensee MAY modify a module or add their own. *PARTIAL: adding is a supported path in the host program (`pcpos module add`): the module stands beside the register, the addition is recorded with the digest of its Specification, and a review selects it by that digest. Modifying a released module is not a supported path: a changed file is found, its real digest is used, and the register records that the method text at hand differs from the release.*

**M-8.** A module that is not a released module MUST NOT carry the identifier of one. *PARTIAL: the host program accepts an added module only under an identifier beginning `local.`, refuses one that takes the name of a module of the method, and serves no text for an identifier that two folders claim. A folder placed by hand inside the method text itself and named `acr.<new name>` is still listed as a module; the register's method check then reports the text as differing from the release.*

**M-9.** The person who owns the machine MAY replace any program outside the kernel image and MAY rebuild the image. *DESIGN.*

## 5. Languages and trust

The weight a result carries comes from what executed the check and what the record holds about it. It does not come from the name of a language, and no program asks which language something is written in. The difference between languages is expressed once, structurally, as where code of each kind is allowed to run and what it is allowed to hold.

| Tier | What runs there | Rule |
| --- | --- | --- |
| **T0** | The kernel image | Rust, no standard library, uncheckable code only in the one named module |
| **T1** | System services: registrar, store, the host program | Rust, uncheckable code refused at compile time |
| **T2** | Contained native code: C, C++, assembly, vendor drivers, existing programs | Runs only inside a compartment, holding only the capabilities it was handed |
| **T3** | Interpreted and managed code: Python and others | The interpreter is a T2 program. A script is data it was given |

**L-1.** A record MUST state what produced each observation it holds: which participant, and whether the host observed it or a participant reported it. *BUILT: actor, authority, `read_by`, `observed` and the task worker are recorded. Since build 18 a check says how the record came by it: `HOST_OBSERVED_TOOL` where the host saw the shell command run and passed on its exit status and output, `REPORTED_BY_PARTICIPANT` where a participant gave its own account, which is all that builds before 18 could record. The host's observation is of the command and its output as the host reported them; it is not an observation that the command tests anything. Since build 19 the observation must have been made after the open review was commissioned, a finding that rests only on reported checks is marked `execution: UNKNOWN`, and material that reaches the record while a review is open is recorded by how it came (words the host did not see the person type, and a file written since commissioning, as PRODUCED). Since build 20 a report from the host is kept only if it says, as true or false, whether the command was interrupted, a report of failure that names exit status 0 is not kept, and a file counts as given to a review only if its last write is in an earlier second than the commissioning: times are recorded to the second, so a write in the commissioning second is not shown to be earlier. The observations are kept in an ordinary file that a shell command can write: none of this is authentication.*

**L-2.** System services MUST NOT hold a capability to an interpreter or a compiler. *PARTIAL: the host program depends on no interpreter and runs none, clause lookup included. Capabilities between services do not exist yet.*

This is why the system does not use Python for its own checks: not by a rule against Python, but because nothing in T0 or T1 can reach it.

**L-3.** Where two implementations of the core disagree, the Rust implementation MUST govern. *BUILT: the Python implementation is run against 17,113 cases published by the Rust one.*

**L-5.** A finding MUST NOT be recorded as supported or as refuted where all that it cites is text the review itself produced. *BUILT: both implementations refuse it (Core 4.3, 5.2), for an artifact recorded as produced after the review was commissioned and for what a task of the review returned. What the guards cannot do is tell a claim that a procedure settles from one that reading settles: the participant declares the kind of claim, and the status shows for each finding whether a check was run and who saw it.*

**L-4.** Code in C or C++ MUST run in T2. *DESIGN.*

C and C++ are used where Rust does not fit: existing device code, vendor drivers, programs that already exist. Their memory errors are then contained by the compartment and not by the hope that there are none.

**L-5.** The core MUST have one stable binary interface, in the C calling convention, and every binding for another language MUST be a thin layer over it that adds no rule. *DESIGN: today the core is reached as a Rust library, through the tool server and at a terminal. The Python implementation is a second implementation, not a binding, and is to become one.*

## 6. Interfaces

**I-1.** Every way of reaching a register MUST go through the same core and the same guards. *BUILT: the terminal commands, the tool server and the hooks are one program over one core.*

**I-2.** A caller MUST NOT be able to name the actor of an event. *BUILT.*

**I-3.** The interface for models MUST be the Model Context Protocol. *BUILT: revisions 2024-11-05 to 2025-11-25; tools only.*

**I-4.** Material retrieved from outside MUST be recorded with where and when it was retrieved. *BUILT in the guards.*

There is deliberately no separate application for connecting PCP OS to other programs' interfaces. A second gateway would be a second place where the rule about who is acting has to be right. Integration is done by speaking to the registrar through one of its interfaces: the tool server for models, the binary interface of L-5 for programs.

## 7. Extensions

PCP OS is meant to be extended and changed by the person who holds it. The question is how to allow that without the two failures other systems show: extensions that run with the system's own authority, and extensions that nobody can account for.

**X-1.** An extension MUST be a package with a manifest stating its identity, its digest, its tier and the capabilities it asks for. *DESIGN. The two PCP plugins for Claude are packages of this shape for another host.*

**X-2.** An extension MUST NOT run in kernel mode. *DESIGN.*

**X-3.** An extension MUST receive only the capabilities the person granted it, and installing, granting and revoking MUST each be recorded. *DESIGN.*

**X-4.** Revoking a capability MUST take back everything derived from it, in every holder. *BUILT in the core.*

What other systems teach, stated from memory and not checked for this document: kernels that load third-party modules into kernel mode inherit those modules' faults as their own; signing a driver establishes who shipped it, not that it is safe; systems that keep drivers and services outside the kernel and hand them capabilities contain such faults; an extension model with no containment turns every extension into a supply-chain risk.

## 8. Devices, display, time and first start

**D-1.** The system MUST use the display at its native resolution as the firmware reports it, and SHOULD choose the default scale from the display's physical density. *PARTIAL: the kernel now drives a display, but an emulated one that has no native resolution: the kernel sets 1024 by 768 for its own screens and whatever mode a program asks for (Doom asks for 640 by 400). Nothing is read from firmware and nothing is scaled.*

**D-2.** Default backgrounds MUST be drawn at the display's own resolution from a resolution-independent description, and MUST use the design tokens of the edition and the workspace. *PARTIAL: the backgrounds exist as vector files generated from the tokens. The kernel's own screens take their colours from the same tokens and draw the emblem from its geometry, but do not show the vector backgrounds.*

**D-3.** Every record MUST carry time in UTC. *BUILT.*

**D-4.** The time zone MUST be a setting of each person, used for display only, and setting it MUST NOT need a network. *DESIGN.*

**D-5.** First start MUST create the person who directs the system before anything else is allowed, and MUST NOT require an account with any service. *DESIGN.*

Intended order of first start: verify; name the person; read the edition; ask language, keyboard and time zone once, proposing what the firmware states; optionally set a workspace; become OPERATIONAL. Each step is an event.

**D-6.** A graphics processor MUST be a device like any other: used by whoever holds a capability to it, absent without consequence. *PARTIAL: the display is such a device (a program without the capability cannot open it, and a kernel started without a display runs its programs and says so). There is no graphics processor driver: every picture is drawn by the processor.*

**D-7.** A program MUST NOT reach a device except through a capability the session granted to it, and what was granted MUST be visible before the program starts. *BUILT: the launcher lists each program's grants; a probe program is run with and without them and each refusal is counted.*

**D-8.** A picture MUST NOT be shown while it is being drawn. *BUILT: two pages; the kernel switches between them after a complete picture.*

**D-9.** One key combination MUST belong to the kernel alone and MUST end the running program. *BUILT: Ctrl+Alt+Del, tested against a program that never yields.*

**D-10.** Sound MUST NOT stop a program that cannot be heard: with no sound device, or a full buffer, the program continues. *BUILT: writes block only as long as the device drains; without a device the files do not exist and the program runs silent.*

## 8a. The test program

Doom was chosen because it was written for another system thirty years ago, is exact about time, and uses every device a session has. It is not part of PCP OS. Its source is not in this release: the Source folder holds a build script that fetches two public projects at fixed commits (doomgeneric and Chocolate Doom, both under the GNU General Public License, version 2), together with the six small files written here that connect them to the kernel's files under `/dev`. The result is a separate program in its own address space, started like any other. No game data is included either.

What was measured, in QEMU 8.2.2 without hardware virtualisation, is in the kernel's README and in the audit record of build 21: the speed with the limiter off, the speed of normal play against the game's own clock, whether sound ever ran dry, how late music was, a long run, saving and loading, and return to the launcher. What was not: any real computer, any real display, keyboard or sound device, and the judgement of a person playing it.

## 8b. Instruments

An instrument is a calculation that a module of ACR names and a program carries out, so that the answer does not depend on anyone's arithmetic (ACR Core 14.1). PCP OS's program runs eight: exact rational arithmetic, whether a reported mean is possible for its sample size, whether a percentage is possible for its denominator, conversion and comparison of units, a digest of a file, the order of dated events, the coverage of a checklist, and the counts of a Calibration. Each run is recorded as a check the program ran itself, which is the only way a claim in a register can come to rest on something executed rather than stated. An instrument checks arithmetic and form. It does not know whether the numbers it was given are true.

The modules are the reason the system is built this way round: each one names what in its field can be checked by machine, and the system is where such a check can be run by something other than the party making the claim. Eight instruments is a beginning. What each module still only names is listed in the module's own Instruments section.

**The kernel is itself an instrument, and the test of this build shows how.** When the kernel runs a program it records, in its own register, things the program has no means to alter: the digest of the file before any of it ran, what the session granted, the time by the kernel's clock, how many pictures reached the display, how often the sound device ran dry, how late a music write was, how many frames were taken and whether all came back, and how the process ended. None of these is the program's statement about itself. "Doom plays at the right speed with sound that never ran dry" is therefore a claim with an executed check under it and a record of who observed what, which is the form ACR asks every claim to take.

What that makes possible for each module, and how much of it exists:

| Module | What the system can observe that a participant can only state | Exists now | Not yet |
|---|---|---|---|
| Mathematics | The result of a calculation, computed by the system from the inputs recorded | Exact rational arithmetic in the host program | A proof checker; arithmetic in the kernel's own interface |
| Statistics | Whether reported figures are possible at all for the sample size given; counts taken by the system instead of reported to it | Two consistency checks in the host program; the kernel's own counts of a process | Recomputation of an analysis from recorded data |
| Science | The order in which things happened, by a clock the examined party does not set; that a prediction was recorded before its outcome | Ordering of dated events in the host program; every kernel event carries the kernel's time | A source of time the kernel can vouch for across a stop; registration of a prediction as a sealed record |
| Engineering | Measured behaviour against a stated requirement, in stated units | Unit conversion and comparison in the host program; the kernel's measurements of a running program | Requirements held as records that a measurement is compared with automatically |
| Cryptography | The digest of exactly what was run or read | Digests by the host program; the kernel measures every program before running it, and its own image | A signature check; anything to compare the kernel's own measurement with |
| Quality | Which items of a checklist have an executed check under them | Coverage of a checklist in the host program; the kernel's self-tests as a table | A review's criteria held by the system and marked by the checks that ran |

The column on the right is the work this design points at. It is listed so that nothing in the middle column is read as more than it is.

## 8c. Calibration

Calibration is the measurement of whether a participant took ACR in: counts of what was acquired, what can be retrieved, and what is applied, taken by someone other than the participant (ACR Core 14.4). The system's part is small and exact: it holds the Calibration Record, it computes the counts with the instrument above, and an access record names the Calibration Record it rests on or says NONE. It never states that anyone is calibrated.

Build 22 changed three things here, each because the first Calibration went wrong in that place (Audit 13). The measuring arrangement of build 21 kept an assistant's file tools out of the plugin's own folder, and the review skill told the assistant to read the kernel text and the list of event bodies from that folder: most reviews could not be begun. The program now serves both itself, as the tools `acr_kernel` and `acr_events`, so a session that cannot open the folder loses nothing. A Calibration Record now states its confinement, what the measurement withheld from the participant or gave it beyond the delivery as used, and the instrument refuses a supported Claim of onboarding in a record that does not. And a record may carry a second reading of its application runs, which the instrument checks covers no more runs than were read.

## 9. Security

What is defended, and what is not.

| Threat | Today |
| --- | --- |
| A model or program writing events it has no right to, through the interfaces | Refused: actor fixed per session; person's and system's events not accepted from a session |
| A model getting its own reading of a prompt recorded as the person's act | Refused since build 13: only a typed command is the person's |
| Reaching a file outside the served directory | Refused by name and by the opened object's real location (Linux); a race can still create an empty file outside |
| Rewriting the register file directly | Not prevented on another operating system. Detected only by someone who kept the head digest |
| Running the host program with made-up input | Not prevented on another operating system |
| Corrupting kernel memory through a kernel bug | Limited by K-3 to K-7; not by address randomization or stack canaries |
| A changed kernel image | Measured, not compared (K-12) |
| Speculative-execution attacks | No mitigation beyond the processor's own |
| Devices that write memory directly | No protection |
| Physical access | No protection |

| Mitigation | Status |
| --- | --- |
| Writable or executable, never both | BUILT |
| Least mapping; guard pages; separate stacks for three exceptions | BUILT |
| Read-only processor tables | BUILT |
| Supervisor execution and access prevention | BUILT where offered; user memory now exists, and the kernel reads a program's bytes through its own mapping and never through a user address |
| User mode: separate address spaces, one system-call gate, no device ports, preemption, a budget | BUILT, as self-tests in the emulator |
| Privilege separation by capability | BUILT in the core; PARTIAL in the kernel (one object, one right) |
| Address randomization; stack canaries; randomness | DESIGN |
| Signed image; measured start | DESIGN |
| Containment of native code (T2) | DESIGN |

## 10. Order of work

One thing at a time, each finished and checked before the next: address spaces and user mode (done in build 17); a program format and a loader (done in build 21, with drivers for an emulated display, keyboard and sound); storage that survives a stop; capability-checked message passing; the registrar as the first service, with storage; start from UEFI with a framebuffer; a measured start; the binary interface of L-5; compartments for T2.
