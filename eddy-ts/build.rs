use std::path::PathBuf;

fn main() {
    // C
    let dir: PathBuf = ["tree-sitter-c", "src"].iter().collect();
    cc::Build::new()
        .include(&dir)
        .file(dir.join("parser.c"))
        .compile("tree-sitter-c");

    // Go
    let dir: PathBuf = ["tree-sitter-go", "src"].iter().collect();
    cc::Build::new()
        .include(&dir)
        .file(dir.join("parser.c"))
        .compile("tree-sitter-go");

    // HTML
    let dir: PathBuf = ["tree-sitter-html", "src"].iter().collect();
    cc::Build::new()
        .include(&dir)
        .file(dir.join("parser.c"))
        // .file(dir.join("scanner.cc"))
        .compile("tree-sitter-html");

    // Javascript
    let dir: PathBuf = ["tree-sitter-javascript", "src"].iter().collect();
    cc::Build::new()
        .include(&dir)
        .file(dir.join("parser.c"))
        .file(dir.join("scanner.c"))
        .compile("tree-sitter-javascript");

    // Rust
    let dir: PathBuf = ["tree-sitter-rust", "src"].iter().collect();
    cc::Build::new()
        .include(&dir)
        .file(dir.join("parser.c"))
        .file(dir.join("scanner.c"))
        .compile("tree-sitter-rust");
}
