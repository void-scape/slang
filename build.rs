use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let tests_dir = Path::new(&manifest_dir).join("test");
    let out_dir = env::var("OUT_DIR").unwrap();
    let dest = Path::new(&out_dir).join("slang_tests.rs");
    println!("cargo:rerun-if-changed={}", tests_dir.display());
    let mut generated = String::new();
    let files = collect_files(&tests_dir);
    for path in files.iter() {
        let rel = path.strip_prefix(&tests_dir).unwrap();
        let test_name = rel.to_string_lossy().replace('.', "_");
        let mut capture = path.clone();
        capture.set_extension("txt");
        let capture = capture.to_string_lossy();
        let path = path.to_string_lossy();
        generated.push_str(&format!(
            r#"#[test]fn {test_name}() {{crate::test::compile_and_run("{path}", "{capture}");}}"#
        ));
    }
    fs::write(&dest, generated).unwrap();
}

fn collect_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    visit(dir, &mut files);
    files.sort();
    files
}

fn visit(dir: &Path, acc: &mut Vec<std::path::PathBuf>) {
    for entry in fs::read_dir(dir).unwrap().flatten() {
        let path = entry.path();
        if path.is_dir() {
            visit(&path, acc);
        } else if path.extension().and_then(|e| e.to_str()) == Some("sl") {
            acc.push(path);
        }
    }
}
