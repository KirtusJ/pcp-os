//! Exhaustive checks of the state machines, the task table, capabilities across holders and the authority model.
use pcpos_core::authority::{authority_of, authority_of_host, change_authorized, Edition, Kind, Roles, Standard, Violation};
use pcpos_core::cap::{CapError, CapSpace, Rights};
use pcpos_core::machine::{self, *};
use pcpos_core::task::{Priority, TaskError, TaskTable, Worker};
use std::fs;
use std::path::PathBuf;

#[test]
fn every_table_is_well_formed() {
    for m in machine::ALL.iter() {
        assert_eq!(m.defects(), 0, "{}", m.name);
    }
}

#[test]
fn the_tables_equal_the_published_tables_byte_for_byte() {
    let mut text = String::new();
    machine::export(&mut text).unwrap();
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../vectors/machines.tsv");
    assert_eq!(text, fs::read_to_string(p).unwrap());
}

#[test]
fn the_system_can_always_reach_safe_and_never_leaves_it_except_by_stopping() {
    for s in System::ALL {
        match s {
            System::Stopped => assert!(s.step(SystemEvent::IntegrityFault).is_err()),
            System::Stopping => assert_eq!(s.step(SystemEvent::IntegrityFault), Ok(System::Stopped)),
            _ => assert_eq!(s.step(SystemEvent::IntegrityFault), Ok(System::Safe), "{:?}", s),
        }
    }
    for e in SystemEvent::ALL {
        match System::Safe.step(*e) {
            Ok(n) => assert!(matches!(n, System::Safe | System::Stopping), "{:?}", e),
            Err(_) => {}
        }
    }
    // Operation is entered only from VERIFY, and after maintenance the system verifies again.
    for s in System::ALL {
        for e in SystemEvent::ALL {
            if s.step(*e) == Ok(System::Operational) {
                assert!((*s == System::Verify && *e == SystemEvent::VerifiedOk) || (*s == System::Degraded && *e == SystemEvent::ServiceRestored));
            }
        }
    }
    assert_eq!(System::Maintenance.step(SystemEvent::MaintenanceEnd), Ok(System::Verify));
    // Shutdown is accepted wherever anything runs.
    for s in System::ALL {
        if !matches!(s, System::Stopping | System::Stopped) {
            assert_eq!(s.step(SystemEvent::Shutdown), Ok(System::Stopping));
        }
    }
}

#[test]
fn a_halt_is_never_refused_and_always_ends_use() {
    for s in Use::ALL {
        assert_eq!(s.step(UseEvent::Halt), Ok(Use::NotInUse));
    }
    for r in Review::ALL {
        match cascade_of_halt(*r) {
            Some(ev) => assert_eq!(r.step(ev), Ok(Review::ClosedIncomplete)),
            None => assert!(matches!(r, Review::ClosedComplete | Review::ClosedIncomplete | Review::Aborted)),
        }
    }
    // A halt is permitted in every system state in which work can be going on.
    for s in [System::Operational, System::Degraded, System::Maintenance, System::Safe] {
        for u in Use::ALL {
            assert_eq!(permits(s, *u, Class::Halt), Ok(()));
        }
    }
}

#[test]
fn a_sealed_session_accepts_only_a_successor_session() {
    for r in [Review::ClosedComplete, Review::ClosedIncomplete, Review::Aborted] {
        for e in ReviewEvent::ALL {
            assert_eq!(r.step(*e).is_ok(), *e == ReviewEvent::OpenSession, "{:?} {:?}", r, e);
        }
    }
    // A paused session is not completed without being resumed.
    assert!(Review::Paused.step(ReviewEvent::CloseComplete).is_err());
}

#[test]
fn a_task_is_done_only_by_a_verified_result() {
    for s in Task::ALL {
        for e in TaskEvent::ALL {
            if s.step(*e) == Ok(Task::Done) {
                assert!(*s == Task::Exited && *e == TaskEvent::ResultVerified);
            }
        }
    }
    // The watchdog reaches every state that is not final, and a stop can always be asked for while a task lives.
    for s in Task::ALL {
        assert_eq!(s.step(TaskEvent::Deadline).is_ok(), !s.is_final());
        if !s.is_final() && *s != Task::Exited {
            assert!(s.step(TaskEvent::StopRequest).is_ok(), "{:?}", s);
        }
    }
}

#[test]
fn review_steps_need_use_and_an_operating_system() {
    for s in System::ALL {
        for u in Use::ALL {
            let ok = permits(*s, *u, Class::Review).is_ok();
            assert_eq!(ok, matches!(s, System::Operational | System::Degraded) && *u == Use::InUse);
            assert_eq!(permits(*s, *u, Class::Host).is_ok(), *s != System::Stopped);
            assert_eq!(permits(*s, *u, Class::Direction).is_ok(), matches!(s, System::Operational | System::Degraded));
            assert_eq!(permits(*s, *u, Class::Task).is_ok(), matches!(s, System::Operational | System::Degraded));
        }
    }
    for e in SystemEvent::ALL {
        let stops = cascade_of_system(*e).is_some();
        assert_eq!(stops, matches!(e, SystemEvent::IntegrityFault | SystemEvent::VerifyFailed | SystemEvent::Shutdown | SystemEvent::MaintenanceBegin));
    }
}

#[test]
fn committed_gates_are_always_an_ordered_prefix() {
    let mut sets = 0;
    for required in 0u8..64 {
        let valid = required & MINIMUM_GATES == MINIMUM_GATES;
        if !valid {
            assert_eq!(gate_step(required, 0, 0), Err(GateRefusal::BadRequiredSet));
            continue;
        }
        sets += 1;
        // From every reachable committed set, exactly one gate is accepted: the lowest required one not yet committed.
        let mut passed = 0u8;
        loop {
            let accepted: Vec<u8> = (0..6).filter(|g| gate_step(required, passed, *g).is_ok()).collect();
            if passed == required {
                assert!(accepted.is_empty());
                break;
            }
            assert_eq!(accepted.len(), 1);
            let g = accepted[0];
            assert_eq!(g, (required & !passed).trailing_zeros() as u8);
            for other in 0..6u8 {
                if other != g {
                    let want = if required & (1 << other) == 0 { GateRefusal::NotRequired } else if passed & (1 << other) != 0 { GateRefusal::AlreadyCommitted } else { GateRefusal::Before(g) };
                    assert_eq!(gate_step(required, passed, other), Err(want));
                }
            }
            passed = gate_step(required, passed, g).unwrap();
        }
    }
    assert_eq!(sets, 8);
}

#[test]
fn the_task_table_holds_its_rules() {
    let mut t: TaskTable<4> = TaskTable::new();
    let a = t.admit("verify-baseline", Worker::Program, 1, 0, 10, 100).unwrap();
    let b = t.admit("challenge", Worker::Model, 1, 0, 10, 100).unwrap();
    let c = t.admit("other", Worker::Model, 2, 0, 10, 100).unwrap();
    assert_eq!(t.admit("bad label", Worker::Program, 0, 0, 1, 1), Err(TaskError::BadLabel));
    for id in [a, b, c] {
        t.event(id, TaskEvent::Start, 1).unwrap();
    }
    // A task's own report of success is EXITED; only the digest read back makes it DONE.
    assert_eq!(t.exit_ok(a, [7; 32], 2), Ok(Task::Exited));
    assert_eq!(t.get(a).unwrap().attention(), None);
    assert_eq!(t.settle(a, Some([7; 32])), Ok(Task::Done));
    // A task that reports success and whose result is absent, or differs, has FAILED.
    let d = t.admit("claims-much", Worker::Model, 2, 2, 10, 100).unwrap();
    t.event(d, TaskEvent::Start, 2).unwrap();
    t.exit_ok(d, [1; 32], 3).unwrap();
    assert_eq!(t.settle(d, Some([2; 32])), Ok(Task::Failed));
    assert_eq!(t.admit("fifth", Worker::Program, 0, 3, 1, 1), Err(TaskError::Full));
    // Silence past the quiet limit is LOST, and LOST asks for a response now.
    t.event(c, TaskEvent::Heartbeat, 8).unwrap();
    assert_eq!(t.sweep(12), 1);
    assert_eq!(t.get(b).unwrap().state, Task::Lost);
    assert_eq!(t.get(b).unwrap().attention(), Some(Priority::High));
    assert_eq!(t.get(c).unwrap().state, Task::Running);
    // Spending past the budget asks the task to stop.
    assert_eq!(t.charge(c, 60, 13), Ok(true));
    assert_eq!(t.charge(c, 60, 14), Ok(false));
    assert_eq!(t.get(c).unwrap().state, Task::Stopping);
    assert_eq!(t.event(c, TaskEvent::StopConfirmed, 15), Ok(Task::Stopped));
    assert_eq!(t.attention(), (1, 1, 1));
    // Rows are released only when final; a sealed table admits nothing.
    assert_eq!(t.release(a), Ok(()));
    let e = t.admit("late", Worker::Program, 3, 20, 5, 1).unwrap();
    assert_eq!(t.release(e), Err(TaskError::NotFinal));
    assert_eq!(t.would_stop(0), 1);
    assert_eq!(t.stop_all(0), 1);
    assert_eq!(t.get(e).unwrap().state, Task::Stopped);
    t.release(e).unwrap();
    t.seal();
    assert_eq!(t.admit("after-seal", Worker::Program, 0, 21, 1, 1), Err(TaskError::Sealed));
}

#[test]
fn revocation_reaches_every_holder() {
    let mut s = CapSpace::new(32);
    let (principal, registrar, agent_a, agent_b) = (1, 2, 3, 4);
    let root = s.install(principal, 100, Rights::READ.with(Rights::APPEND).with(Rights::GRANT)).unwrap();
    let reg = s.grant(principal, root, registrar, Rights::READ.with(Rights::APPEND).with(Rights::GRANT)).unwrap();
    let a = s.grant(registrar, reg, agent_a, Rights::APPEND).unwrap();
    let b = s.grant(registrar, reg, agent_b, Rights::READ).unwrap();
    // A handle is valid only in the hands of its holder, and rights never widen on the way down.
    assert_eq!(s.check(agent_b, a, Rights::APPEND), Err(CapError::Invalid));
    assert_eq!(s.check(agent_a, a, Rights::READ), Err(CapError::Denied));
    assert_eq!(s.grant(registrar, reg, agent_a, Rights::DIRECT), Err(CapError::WouldWiden));
    assert_eq!(s.grant(agent_a, a, agent_b, Rights::APPEND), Err(CapError::Denied));
    // Stopping one agent takes back what it held and nothing else.
    assert_eq!(s.revoke_holder(agent_b), 1);
    assert_eq!(s.check(agent_b, b, Rights::READ), Err(CapError::Invalid));
    assert_eq!(s.check(agent_a, a, Rights::APPEND), Ok(100));
    // Revoking the registrar's grant reaches the agents it handed to.
    assert_eq!(s.revoke(agent_a, reg), Err(CapError::Invalid)); // an agent cannot revoke what it was derived from
    assert_eq!(s.revoke(principal, reg), Ok(2));
    assert_eq!(s.check(agent_a, a, Rights::APPEND), Err(CapError::Invalid));
    assert_eq!(s.check(principal, root, Rights::APPEND), Ok(100));
    // A reused slot does not revive an old handle.
    let again = s.grant(principal, root, agent_a, Rights::READ).unwrap();
    assert_ne!(again, a);
    assert_eq!(s.check(agent_a, a, Rights::READ), Err(CapError::Invalid));
    assert_eq!(s.held_by(agent_a), 1);
}

#[test]
fn authority_invariants_hold_for_every_combination() {
    let forbidden_to_non_persons = Rights::DIRECT.with(Rights::ADMIN).with(Rights::SYSTEM);
    let mut granted = 0;
    for edition in [Edition::Personal, Edition::Commercial, Edition::Enterprise] {
        for kind in [Kind::Human, Kind::Program, Kind::Model] {
            for bits in 0u16..128 {
                let roles = Roles(bits);
                match authority_of(kind, roles, edition) {
                    Err(Violation::NoRole) => assert_eq!(bits, 0),
                    Err(_) => {}
                    Ok(a) => {
                        granted += 1;
                        let all = a.union();
                        // No model-driven process and no program is handed the means to start, administer or change.
                        if kind != Kind::Human {
                            assert_eq!(all.0 & forbidden_to_non_persons.0, 0, "{:?} {:?}", kind, roles);
                        }
                        // An auditor's session cannot append, stop, halt or change anything.
                        if roles.has(Roles::AUDITOR) {
                            assert_eq!(all, Rights::READ);
                        }
                        // The means to start imply the means to stop, never the reverse requirement.
                        if a.direction.contains(Rights::DIRECT) {
                            assert!(a.direction.contains(Rights::HALT));
                        }
                        // In the editions for several people, whoever administers does not assess.
                        if edition != Edition::Personal && a.administration.contains(Rights::ADMIN) {
                            assert!(!roles.has(Roles::DIRECTOR) && !roles.has(Roles::REVIEWER));
                        }
                    }
                }
            }
        }
    }
    assert!(granted > 0);
    // Groups add roles and nothing else; the checks run on the union.
    let eff = Roles::effective(Roles::REVIEWER, &[Roles::AUDITOR]);
    assert_eq!(authority_of(Kind::Human, eff, Edition::Commercial), Err(Violation::AuditorWrites));
    // The host can always halt; under human adoption it cannot adopt.
    assert!(authority_of_host(&Standard::BASELINE).direction.contains(Rights::DIRECT));
    let host = authority_of_host(&Standard::CRITICAL);
    assert!(host.direction.contains(Rights::HALT) && !host.direction.contains(Rights::DIRECT));
    // Standards are ordered; the critical one is at least as strict as the baseline and not the reverse.
    assert!(Standard::CRITICAL.at_least(&Standard::BASELINE) && !Standard::BASELINE.at_least(&Standard::CRITICAL));
    assert!(!Standard::CRITICAL.available_in(Edition::Personal) && Standard::CRITICAL.available_in(Edition::Enterprise));
    // Two different people, each with the right, under dual authorization.
    let p = authority_of(Kind::Human, Roles::PRINCIPAL, Edition::Enterprise).unwrap();
    let o = authority_of(Kind::Human, Roles::OPERATOR, Edition::Enterprise).unwrap();
    assert!(change_authorized(&Standard::BASELINE, (1, &p), None));
    assert!(!change_authorized(&Standard::CRITICAL, (1, &p), None));
    assert!(!change_authorized(&Standard::CRITICAL, (1, &p), Some((1, &p))));
    assert!(!change_authorized(&Standard::CRITICAL, (1, &p), Some((2, &o))));
    assert!(change_authorized(&Standard::CRITICAL, (1, &p), Some((2, &p))));
}
