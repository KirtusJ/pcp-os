//! Host tests of which loaded file is a worker that is not listed as a program. From this crate's folder:
//!   rustc --edition=2021 --test tests/test_paired.rs -o /tmp/test-paired && /tmp/test-paired
#[path = "../src/session/paired.rs"]
mod paired;
use paired::runs_only_beside;

const FRONT: &str = "aaaa";
const WORKER: &str = "bbbb";
const OTHER: &str = "cccc";

#[test]
fn a_worker_named_for_a_loaded_program_is_not_listed() {
    let line = "auto=x worker_for=aaaa worker_image=bbbb then=menu";
    assert!(runs_only_beside(Some(line), WORKER, &[FRONT, WORKER, OTHER]));
}

#[test]
fn the_program_it_is_for_and_every_other_file_stay_listed() {
    let line = "worker_for=aaaa worker_image=bbbb";
    assert!(!runs_only_beside(Some(line), FRONT, &[FRONT, WORKER, OTHER]));
    assert!(!runs_only_beside(Some(line), OTHER, &[FRONT, WORKER, OTHER]));
}

#[test]
fn a_cut_line_hides_nothing() {
    assert!(!runs_only_beside(None, WORKER, &[FRONT, WORKER]));
}

#[test]
fn a_worker_whose_program_was_not_loaded_stays_listed() {
    let line = "worker_for=aaaa worker_image=bbbb";
    assert!(!runs_only_beside(Some(line), WORKER, &[WORKER, OTHER]));
}

#[test]
fn half_a_pairing_hides_nothing() {
    assert!(!runs_only_beside(Some("worker_image=bbbb"), WORKER, &[FRONT, WORKER]));
    assert!(!runs_only_beside(Some("worker_for=aaaa"), WORKER, &[FRONT, WORKER]));
    assert!(!runs_only_beside(Some(""), WORKER, &[FRONT, WORKER]));
}

#[test]
fn a_file_named_as_its_own_worker_stays_listed() {
    // The kernel does not pair a program with itself (see `run_entry`), so it is not hidden either.
    assert!(!runs_only_beside(Some("worker_for=bbbb worker_image=bbbb"), WORKER, &[WORKER]));
}

#[test]
fn a_name_that_only_begins_like_the_option_is_not_the_option() {
    assert!(!runs_only_beside(Some("worker_for=aaaa worker_images=bbbb"), WORKER, &[FRONT, WORKER]));
    assert!(!runs_only_beside(Some("xworker_for=aaaa worker_image=bbbb"), WORKER, &[FRONT, WORKER]));
}

#[test]
fn the_first_of_a_repeated_option_is_the_one_read() {
    // As everywhere else on the loader's line.
    assert!(runs_only_beside(Some("worker_for=aaaa worker_image=bbbb worker_image=cccc"), WORKER, &[FRONT, WORKER, OTHER]));
    assert!(!runs_only_beside(Some("worker_for=aaaa worker_image=bbbb worker_image=cccc"), OTHER, &[FRONT, WORKER, OTHER]));
}

#[test]
fn a_worker_for_a_file_that_is_not_a_program_stays_listed() {
    // Director 2's case. The loader handed over a document whose digest the line names as the
    // program the worker is for. It is among the loader's files and not among the programs, so
    // nothing can start it, and the worker is the only thing a person could choose. The caller
    // passes the digests of the programs only: the document's is not there.
    let line = "worker_for=aaaa worker_image=bbbb";
    let programs_only = [WORKER, OTHER];
    assert!(!runs_only_beside(Some(line), WORKER, &programs_only));
    // And where the same digest IS a program, the worker is hidden: the only difference is that.
    assert!(runs_only_beside(Some(line), WORKER, &[FRONT, WORKER, OTHER]));
}
