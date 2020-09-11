use crate::Language;

extern "C" {
    pub fn tree_sitter_c() -> Language;
}
extern "C" {
    pub fn tree_sitter_go() -> Language;
}
extern "C" {
    pub fn tree_sitter_html() -> Language;
}
extern "C" {
    pub fn tree_sitter_javascript() -> Language;
}
extern "C" {
    pub fn tree_sitter_rust() -> Language;
}

// C
pub fn c() -> Language {
    unsafe { tree_sitter_c() }
}
pub const C_HIGHLIGHTS: &str = include_str!("../tree-sitter-c/queries/highlights.scm");
pub const C_INJECTIONS: &str = include_str!("../tree-sitter-c/queries/highlights.scm");

// Go
pub fn go() -> Language {
    unsafe { tree_sitter_go() }
}
pub const GO_HIGHLIGHTS: &str = include_str!("../tree-sitter-go/queries/highlights.scm");
pub const GO_INJECTIONS: &str = include_str!("../tree-sitter-go/queries/highlights.scm");

// HTML
pub fn html() -> Language {
    unsafe { tree_sitter_html() }
}
pub const HTML_HIGHLIGHTS: &str = include_str!("../tree-sitter-html/queries/highlights.scm");
pub const HTML_INJECTIONS: &str = include_str!("../tree-sitter-html/queries/highlights.scm");

// Javascript
pub fn javascript() -> Language {
    unsafe { tree_sitter_javascript() }
}
pub const JS_HIGHLIGHTS: &str = include_str!("../tree-sitter-javascript/queries/highlights.scm");
pub const JS_INJECTIONS: &str = include_str!("../tree-sitter-javascript/queries/highlights.scm");

// Rust
pub fn rust() -> Language {
    unsafe { tree_sitter_rust() }
}
pub const RUST_HIGHLIGHTS: &str = include_str!("../tree-sitter-rust/queries/highlights.scm");
pub const RUST_INJECTIONS: &str = include_str!("../tree-sitter-rust/queries/injections.scm");
