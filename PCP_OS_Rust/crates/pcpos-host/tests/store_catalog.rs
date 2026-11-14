use std::process::Command;

#[test]
fn catalog_names_match_the_instrument_list() {
    let catalog = include_str!("../../../../store/instruments/README.md");
    let mut documented = Vec::new();
    for row in catalog.lines().filter(|line| line.starts_with("| `")) {
        let cell = row[1..]
            .split_once('|')
            .expect("missing second table cell")
            .0
            .trim();
        let name = cell
            .strip_prefix('`')
            .and_then(|s| s.strip_suffix('`'))
            .unwrap_or_else(|| panic!("expected one inline-code name in {row}"));
        assert!(
            !name.is_empty() && !name.contains('`'),
            "invalid app name in {row}"
        );
        documented.push(name);
    }

    let output = Command::new(env!("CARGO_BIN_EXE_pcpos"))
        .args(["instrument", "list"])
        .output()
        .expect("could not start pcpos instrument list");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "pcpos instrument list exited {}: {stderr}",
        output.status
    );
    assert!(
        stderr.is_empty(),
        "unexpected diagnostics from pcpos instrument list: {stderr}"
    );
    let stdout = String::from_utf8(output.stdout).expect("instrument list is not UTF-8");
    let mut available = Vec::new();
    for line in stdout.lines() {
        let (name, _) = line
            .split_once(": ")
            .unwrap_or_else(|| panic!("expected an instrument name and description in {line}"));
        available.push(name);
    }
    assert!(
        !available.is_empty(),
        "pcpos instrument list returned no names"
    );

    documented.sort_unstable();
    available.sort_unstable();
    assert_eq!(
        documented, available,
        "published Store catalog differs from pcpos instrument list"
    );

    let front_page = include_str!("../../../../README.md");
    let mut lines = front_page.lines();
    assert!(
        lines.any(|line| line == "## Built-in apps"),
        "front README is missing its Built-in apps section"
    );
    let mut front_apps = Vec::new();
    for row in lines
        .take_while(|line| !line.starts_with("## "))
        .filter(|line| line.starts_with('|'))
    {
        front_apps.extend(row.split('`').skip(1).step_by(2));
    }
    front_apps.sort_unstable();
    assert_eq!(
        front_apps, available,
        "front README app table differs from pcpos instrument list"
    );
    assert!(
        available.windows(2).all(|pair| pair[0] != pair[1]),
        "duplicate app name in pcpos instrument list"
    );
}
