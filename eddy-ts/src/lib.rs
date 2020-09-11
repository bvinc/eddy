pub use tree_sitter::*;

pub mod language;

#[test]
fn test_blah() {
    let language_rust = language::rust();

    let source_code = "use blah; fn test(i: u32) { let mut x = 5; }";
    let mut parser = Parser::new();
    parser.set_language(language_rust).unwrap();
    let tree = parser.parse(source_code, None).unwrap();
    let root_node = tree.root_node();

    assert_eq!(root_node.kind(), "source_file");
    // assert_eq!(root_node.start_position().column, 0);
    // assert_eq!(root_node.end_position().column, 12);

    println!("{}", root_node.to_sexp());
    print_tree(root_node, 0);
}

#[cfg(test)]
fn print_tree(node: Node, level: u32) {
    let mut cur = node.walk();
    println!(
        "{} {} {}-{}",
        (0..level * 4).map(|_| " ").collect::<String>(),
        cur.node().kind(),
        cur.node().start_position().column,
        cur.node().end_position().column
    );
    if cur.goto_first_child() {
        print_tree(cur.node(), level + 1);
    }
    while cur.goto_next_sibling() {
        print_tree(cur.node(), level + 1);
    }
}
