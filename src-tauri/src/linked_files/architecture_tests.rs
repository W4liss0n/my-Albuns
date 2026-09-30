//! Keeps every access to Arquivos vinculados inside `linked_files`. A flow that
//! opens Originals on its own misses the network policies there: that is how
//! the memory estimate kept reading whole JPEGs after the header inspection
//! stopped doing so (2026-09-29).
use std::path::{Path, PathBuf};

/// A pattern that belongs to `linked_files`, and the files allowed to use it
/// with the reason each one keeps it.
struct Rule {
    pattern: &'static str,
    why: &'static str,
    allowed: &'static [(&'static str, &'static str)],
}

const RULES: &[Rule] = &[
    Rule {
        pattern: "ImageReader::open(",
        why: "opens an image file outside linked_files",
        allowed: &[(
            "eye_correction.rs",
            "eye correction reads and replaces the Original it edits, with its own backup rules",
        )],
    },
    Rule {
        pattern: "ImageReader::new(",
        why: "decodes an image outside linked_files",
        allowed: &[(
            "image_processing.rs",
            "decodes preview bytes already held in memory, never a file",
        )],
    },
    Rule {
        pattern: "std::fs::metadata(entry.path())",
        why: "asks the server again for every listed entry; use LinkedFiles::list_folder",
        allowed: &[],
    },
    Rule {
        pattern: "ImageMemoryEstimate::in_plan(",
        why: "estimates memory by reading Originals; use ImageMemoryEstimate::from_headers",
        allowed: &[],
    },
    Rule {
        pattern: ".observe_in_plan(",
        why: "observes an Original outside linked_files; use LinkedFiles::observe",
        allowed: &[],
    },
    Rule {
        pattern: "inspect_media_",
        why: "inspects an Original outside linked_files; use LinkedFiles::header or inspect_decoded",
        allowed: &[],
    },
];

fn sources(folder: &Path, found: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(folder).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            sources(&path, found);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            found.push(path);
        }
    }
}

/// Violations in one source file, given its path relative to `src`.
fn violations(relative: &str, source: &str) -> Vec<String> {
    // The module itself, tests and the network measurement, which may build
    // fixtures however they need.
    if relative == "linked_files.rs"
        || relative == "network_bench.rs"
        || relative.ends_with("tests.rs")
        || relative.contains("/tests/")
    {
        return Vec::new();
    }
    // Unit tests at the end of a file are fixtures too.
    let source = source.replace("\r\n", "\n");
    let production = source
        .split("#[cfg(test)]\nmod tests")
        .next()
        .unwrap_or(&source);
    let name = relative.rsplit('/').next().unwrap_or(relative);
    let mut found = Vec::new();
    for rule in RULES {
        if rule.allowed.iter().any(|(allowed, _)| *allowed == name) {
            continue;
        }
        for (index, line) in production.lines().enumerate() {
            if line.contains(rule.pattern) {
                found.push(format!(
                    "{relative}:{}: {} ({})",
                    index + 1,
                    rule.pattern,
                    rule.why
                ));
            }
        }
    }
    found
}

#[test]
fn only_linked_files_accesses_arquivos_vinculados() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    sources(&root, &mut files);
    let mut found = Vec::new();
    for file in files {
        let relative = file
            .strip_prefix(&root)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        found.extend(violations(
            &relative,
            &std::fs::read_to_string(&file).unwrap(),
        ));
    }
    assert!(found.is_empty(), "{}", found.join("\n"));
}

#[test]
fn a_new_flow_opening_originals_on_its_own_is_caught() {
    let flow = "fn new_flow(path: &Path) {\n    let image = image::ImageReader::open(path);\n}\n\
                #[cfg(test)]\nmod tests {\n    fn fixture() { image::ImageReader::open(\"x\"); }\n}\n";
    assert_eq!(
        violations("new_flow.rs", flow).len(),
        1,
        "only production code counts"
    );
    assert!(
        violations("eye_correction.rs", flow).is_empty(),
        "a listed exception passes"
    );
    assert!(violations("new_flow/tests.rs", flow).is_empty());
    assert_eq!(
        violations(
            "scan.rs",
            "for entry in entries {\n    std::fs::metadata(entry.path());\n    ImageMemoryEstimate::in_plan(plan, []);\n}\n",
        )
        .len(),
        2
    );
}
