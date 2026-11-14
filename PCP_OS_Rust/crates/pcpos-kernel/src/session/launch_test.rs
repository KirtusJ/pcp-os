use super::*;

fn field<'a>(event: &'a Value, key: &str) -> Option<&'a str> {
    event.get("body")?.get(key)?.as_str()
}

fn kind(event: &Value) -> Option<&str> {
    event.get("kind")?.as_str()
}

pub(super) fn run(rec: &mut Record, world: World, tasks: &mut u32, start: &Start, failed: &mut u32) -> World {
    let Some(image) = crate::elf_of("sleeper") else {
        crate::check("the sleeper fixture is present", false, failed);
        return world;
    };
    let header = &image[..64]; // Complete ELF header declaring a program header past the end of the file.
    let refused = Entry { name: "truncated".into(), arguments: String::new(), image: header, digest: sha256_hex(header) };
    let admitted = Entry { name: "sleeper".into(), arguments: String::new(), image: &image, digest: sha256_hex(&image) };
    let before = *tasks;
    let first = rec.ledger().len() as usize;
    let offset = rec.ledger().bytes().len();
    let head = rec.ledger().head().to_string();
    let (_, world) = run_entry(rec, &refused, world, tasks, start, failed);
    let refused_rows = rec.ledger().len() as usize - first;
    let after_refusal = *tasks;
    let (_, world) = run_entry(rec, &admitted, world, tasks, start, failed);

    let Ok((events, _)) = ledger::verify_tail(&rec.ledger().bytes()[offset..], first, &head) else {
        crate::check("the launch register tail verifies", false, failed);
        return world;
    };
    let (refusal, admission) = events.split_at(refused_rows);
    crate::check("a refused launch records its file and reason without creating a task",
        after_refusal == before && matches!(refusal, [e] if kind(e) == Some("HOST_EVENT")
            && field(e, "event") == Some("PROGRAM_LAUNCH_REFUSED") && field(e, "name") == Some(refused.name.as_str())
            && field(e, "sha256") == Some(refused.digest.as_str()) && field(e, "reason").is_some_and(|reason| !reason.is_empty())), failed);
    let id = format!("T{}", before + 1);
    let output = sha256_hex(&[]); // sleeper exits zero without writing output.
    crate::check("the next admitted launch records creation, output and observed exit on one task",
        *tasks == before + 1 && matches!(admission, [s, a, e] if kind(s) == Some("TASK_STARTED")
            && field(s, "id") == Some(id.as_str()) && field(s, "observed") == Some("PROCESS_CREATED") && field(s, "label") == Some(admitted.name.as_str())
            && kind(a) == Some("ARTIFACT_ADDED") && field(a, "task") == Some(id.as_str()) && field(a, "sha256") == Some(output.as_str())
            && kind(e) == Some("TASK_ENDED") && field(e, "id") == Some(id.as_str()) && field(e, "observed") == Some("PROCESS_END")
            && field(e, "outcome") == Some("DONE") && field(e, "read_back") == Some(output.as_str())), failed);
    world
}
