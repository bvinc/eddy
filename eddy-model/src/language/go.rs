use super::Layer;
use crate::language::capture::Capture;
use crate::language::util::RopeTextProvider;
use crate::Point;
use eddy_ts::{language, InputEdit, Language, Node, Parser, Query, QueryCursor, Tree};
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
        self.node_to_capture.get(&id).copied()
    }
    fn unset_tree(&mut self) {
        self.tree = None;
    }
    fn tree(&self) -> Option<&Tree> {
        self.tree.as_ref()
    }

    fn update_highlights(&mut self, rope: &Rope) {
        self.tree = self.parser.parse_with(
            &mut |byte_idx, _pos| {
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

    fn edit_tree(&mut self, start: Point, old_end: Point, new_end: Point) {
        if let Some(tree) = &mut self.tree {
            tree.edit(&InputEdit {
                start_byte: start.byte,
                old_end_byte: old_end.byte,
                new_end_byte: new_end.byte,
                start_position: eddy_ts::Point {
                    row: start.line,
                    column: start.col,
                },
                old_end_position: eddy_ts::Point {
                    row: old_end.line,
                    column: old_end.col,
                },
                new_end_position: eddy_ts::Point {
                    row: new_end.line,
                    column: new_end.col,
                },
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
