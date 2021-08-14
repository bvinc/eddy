use std::collections::HashMap;

pub struct Layout {
    lines: HashMap<usize, LayoutLine>,
}

impl Layout {
    pub fn new() -> Layout {
        Self {
            lines: HashMap::new(),
        }
    }

    pub fn clear(&mut self) {
        self.lines.clear();
    }

    pub fn insert(&mut self, line_num: usize, line: LayoutLine) {
        self.lines.insert(line_num, line);
    }
}

pub struct LayoutLine {
    items: Vec<LayoutItem>,
}

impl LayoutLine {
    pub fn new() -> LayoutLine {
        Self { items: vec![] }
    }

    pub fn clear(&mut self) {
        self.items.clear();
    }

    pub fn push(&mut self, item: LayoutItem) {
        self.items.push(item);
    }

    /// Go through all of the consecutive items in the line and convert and index to an x value
    pub fn index_to_x(&self, index: usize) -> i32 {
        let mut idx = index;
        let mut x = 0;

        for item in &self.items {
            dbg!(&item.text);
            if item.text.len() <= idx {
                idx -= item.text.len();
                // TODO get rid of this copying once glyphs.width is no longer mut
                let mut glyphs = item.glyphs.clone();
                x += glyphs.width();
                dbg!(x, glyphs.width());
            } else {
                // TODO get rid of this copying once glyphs.width is no longer mut
                let mut glyphs = item.glyphs.clone();
                // This index_to_x method unfortunately requires a &mut
                // Analysis for no reason.  This needs to be fixed.
                // Yes I know transmuting & to &mut is always UB.
                // Yes I know I can't do it.
                // Yes I know I'm not special.
                let x_in_item = glyphs.index_to_x(
                    &item.text,
                    unsafe {
                        &mut *(item.item.analysis() as *const pango::Analysis
                            as *mut pango::Analysis)
                    },
                    idx as i32,
                    false,
                );
                dbg!(x, x_in_item);
                return x + x_in_item;
            }
        }
        return x;
    }
}

pub struct LayoutItem {
    pub text: String,
    pub item: pango::Item,
    pub glyphs: pango::GlyphString,
    pub x_off: i32,
}
