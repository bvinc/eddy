use super::Layer;
use crate::language::capture::Capture;
use crate::language::util::RopeTextProvider;
use eddy_ts::{language, InputEdit, Language, Node, Parser, Point, Query, QueryCursor, Tree};
use ropey::Rope;
use std::borrow::Cow;
use std::collections::HashMap;
use std::fmt;

pub struct GoLayer {
    highlights_query: Query,
    captures_by_id: Vec<Option<Capture>>,
    node_to_capture: HashMap<usize, Capture>,
    parser: Parser,
    tree: Option<Tree>,
}

impl GoLayer {
    pub fn new() -> Self {
        let highlights_query = Query::new(Self::lang(), language::GO_HIGHLIGHTS).unwrap();
        let mut capture_map = HashMap::new();
        let captures_by_id = highlights_query
            .capture_names()
            .iter()
            .map(|cn| Capture::from_name(cn))
            .collect();
        for (id, cn) in highlights_query.capture_names().iter().enumerate() {
            capture_map.insert(id, Capture::from_name(cn));
        }

        let mut parser = Parser::new();
        parser.set_language(Self::lang());

        Self {
            highlights_query,
            captures_by_id,
            node_to_capture: HashMap::new(),
            parser,
            tree: None,
        }
    }
    pub fn lang() -> Language {
        language::go()
    }
    pub fn highlights_query() -> Query {
        Query::new(Self::lang(), language::GO_HIGHLIGHTS).unwrap()
    }
}

impl Layer for GoLayer {
    fn capture(&self, idx: usize) -> Option<Capture> {
        self.captures_by_id.get(idx).and_then(|c| *c)
    }
    fn capture_from_node(&self, id: usize) -> Option<Capture> {
        self.node_to_capture.get(&id).map(|c| *c)
    }
    fn tree(&self) -> Option<&Tree> {
        self.tree.as_ref()
    }

    fn update_highlights(&mut self, rope: &Rope) {
        self.tree = self.parser.parse_with(
            &mut |byte_idx, pos| {
                if byte_idx > rope.len_bytes() {
                    return [].as_ref();
                }
                let (s, chunk_byte_idx, _, _) = rope.chunk_at_byte(byte_idx);
                let ret = &s.as_bytes()[byte_idx - chunk_byte_idx..];
                // println!("asked for {} {}, returned {:?}", byte_idx, pos, ret);
                ret
            },
            self.tree.as_ref(),
        );
        if let Some(tree) = &self.tree {
            // super::print_tree(tree.root_node(), 0);

            self.node_to_capture.clear();

            fn rope_bytes_to_str<'a>(
                rope: &'a Rope,
                range: std::ops::Range<usize>,
            ) -> Cow<'a, str> {
                let start_char = rope.byte_to_char(range.start);
                let end_char = rope.byte_to_char(range.end);
                rope.slice(start_char..end_char).into()
            }

            let mut cursor = QueryCursor::new();
            let captures = cursor.captures(
                &self.highlights_query,
                tree.root_node(),
                RopeTextProvider::new(rope),
            );
            for cap in captures {
                for c in cap.0.captures {
                    if let Some(capture) = self.capture(c.index as usize) {
                        self.node_to_capture.insert(c.node.id(), capture);
                    }
                }
            }
        }
    }

    fn edit_tree(&mut self, rope: &Rope, start: usize, old_end: usize, new_end: usize) {
        // all units here are in code points.
        // tree sitter's "column" is the number of CODE POINTS since the start of the line
        let start_byte = rope.char_to_byte(start);
        let start_line = rope.char_to_line(start);
        let start_col = start - rope.line_to_char(start_line);
        let start_position = Point {
            row: start_line,
            column: start_col,
        };

        let old_end_byte = rope.char_to_byte(old_end);
        let old_end_line = rope.char_to_line(old_end);
        let old_end_col = old_end - rope.line_to_char(old_end_line);
        let old_end_position = Point {
            row: old_end_line,
            column: old_end_col,
        };

        let new_end_byte = rope.char_to_byte(new_end);
        let new_end_line = rope.char_to_line(new_end);
        let new_end_col = new_end - rope.line_to_char(new_end_line);
        let new_end_position = Point {
            row: new_end_line,
            column: new_end_col,
        };

        if let Some(tree) = &mut self.tree {
            tree.edit(&InputEdit {
                start_byte,
                old_end_byte,
                new_end_byte,
                start_position,
                old_end_position,
                new_end_position,
            });
        }
    }
}

impl fmt::Debug for GoLayer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GoLayer")
            .field("highlights_query", &self.highlights_query)
            .field("captures_by_id", &self.captures_by_id)
            .field("node_to_capture", &self.node_to_capture)
            .field("tree", &self.tree)
            .finish()
    }
}
