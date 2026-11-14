use super::*;

fn field<'a>(event: &'a Value, key: &str) -> Option<&'a str> {
    event.get("body")?.get(key)?.as_str()
}

fn kind(event: &Value) -> Option<&str> {
    event.get("kind")?.as_str()
}

/// Read exactly the events appended by one operation from its verified predecessor.
fn capture<T>(rec: &mut Record, operation: impl FnOnce(&mut Record) -> T) -> (T, Vec<Value>) {
    let first = rec.ledger().len() as usize;
    let offset = rec.ledger().bytes().len();
    let head = rec.ledger().head().to_string();
    let result = operation(rec);
    let events = ledger::verify_tail(&rec.ledger().bytes()[offset..], first, &head).expect("launch register tail").0;
    (result, events)
}

fn refused(events: &[Value], entry: &Entry, reason: &str) -> bool {
    matches!(events, [event] if kind(event) == Some("HOST_EVENT") && field(event, "event") == Some("PROGRAM_LAUNCH_REFUSED")
        && field(event, "name") == Some(entry.name.as_str()) && field(event, "sha256") == Some(entry.digest.as_str())
        && field(event, "reason").is_some_and(|s| s.contains(reason)))
}

fn completed(events: &[Value], entry: &Entry, id: &str, label: &str) -> bool {
    let output = sha256_hex(&[]); // The sleeper exits zero without writing output.
    let attempt = format!("program {} of SHA-256 {}", entry.name, entry.digest);
    matches!(events, [s, a, e] if kind(s) == Some("TASK_STARTED") && field(s, "observed") == Some("PROCESS_CREATED")
        && field(s, "id") == Some(id) && field(s, "label") == Some(label) && field(s, "attempt") == Some(attempt.as_str())
        && kind(a) == Some("ARTIFACT_ADDED") && field(a, "task") == Some(id) && field(a, "sha256") == Some(output.as_str())
        && kind(e) == Some("TASK_ENDED") && field(e, "id") == Some(id) && field(e, "observed") == Some("PROCESS_END")
        && field(e, "outcome") == Some("DONE") && field(e, "result") == Some(output.as_str()) && field(e, "read_back") == Some(output.as_str()))
}

pub(super) fn run(rec: &mut Record, world: World, tasks: &mut u32, start: &Start, failed: &mut u32) -> World {
    let Some(image) = crate::elf_of("sleeper") else {
        crate::check("the sleeper fixture is present", false, failed);
        return world;
    };
    let entry = |name: &str, bytes| Entry { name: name.into(), arguments: String::new(), image: bytes, digest: sha256_hex(bytes) };
    let malformed = entry("truncated", &image[..64]);
    let punctuated = entry("bad+name.elf", &image);
    let ordinary = entry("sleeper", &image);
    let before = *tasks;
    let ((_, world), rows) = capture(rec, |r| run_entry(r, &malformed, world, tasks, start, failed));
    crate::check("a refused launch records its file and reason without creating a task",
        *tasks == before && refused(&rows, &malformed, "program header"), failed);

    let (ok, rows) = capture(rec, |r| record_supplied(r, &punctuated.name, &punctuated.digest, punctuated.image.len()));
    crate::check("the supplied file retains its original punctuated name and digest",
        ok && matches!(rows.as_slice(), [e] if kind(e) == Some("ARTIFACT_ADDED") && field(e, "origin") == Some("SUPPLIED")
            && field(e, "filename") == Some(punctuated.name.as_str()) && field(e, "sha256") == Some(punctuated.digest.as_str())), failed);
    let ((_, world), rows) = capture(rec, |r| run_entry(r, &punctuated, world, tasks, start, failed));
    let id = format!("T{}", before + 1);
    crate::check("the punctuated application runs and records its input, output and exit on one task",
        *tasks == before + 1 && completed(&rows, &punctuated, &id, &id), failed);

    // Deliberately offer a stale next ID. The session keeps interrupts off, while the guest sleeps for 50 ms if it runs.
    let mut wrong_count = *tasks + 1;
    let original_count = wrong_count;
    let mut world = world;
    let marker = (String::from("launch-test"), b"kept".to_vec());
    world.store.push(marker.clone());
    let frames = user::frames_free();
    let ticks = arch::ticks();
    let ((_, mut world), rows) = capture(rec, |r| run_entry(r, &ordinary, world, &mut wrong_count, start, failed));
    let kept = world.store.pop() == Some(marker);
    crate::check("a refused task start prevents execution and returns all program frames",
        wrong_count == original_count && arch::ticks() == ticks && user::frames_free() == frames && kept
            && refused(&rows, &ordinary, "register"), failed);

    let ((_, world), rows) = capture(rec, |r| run_entry(r, &ordinary, world, tasks, start, failed));
    let id = format!("T{}", before + 2);
    crate::check("the next admitted launch records creation, output and observed exit on one task",
        *tasks == before + 2 && completed(&rows, &ordinary, &id, &ordinary.name), failed);
    world
}
