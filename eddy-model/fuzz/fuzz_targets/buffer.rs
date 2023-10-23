#![no_main]
use eddy_model::Buffer;
use libfuzzer_sys::arbitrary;
use libfuzzer_sys::arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;

#[derive(Arbitrary, Debug)]
enum BufferMethod {
    Insert { s: String },
    InsertNewline,
    MoveUp,
    MoveDown,
    MoveLeft,
    MoveRight,
    MoveUpAndModifySelection,
    MoveDownAndModifySelection,
    MoveLeftAndModifySelection,
    MoveRightAndModifySelection,
    DeleteForward,
    DeleteBackward,
}

fuzz_target!(|methods: Vec<BufferMethod>| {
    let mut buffer = Buffer::new();
    buffer.init_view(0);
    for method in methods {
        dbg!(&method);
        match method {
            BufferMethod::Insert { s } => buffer.insert(0, &s),
            BufferMethod::InsertNewline => buffer.insert_newline(0),
            BufferMethod::MoveUp => buffer.move_up(0),
            BufferMethod::MoveDown => buffer.move_down(0),
            BufferMethod::MoveLeft => buffer.move_left(0),
            BufferMethod::MoveRight => buffer.move_right(0),
            BufferMethod::MoveUpAndModifySelection => buffer.move_up_and_modify_selection(0),
            BufferMethod::MoveDownAndModifySelection => buffer.move_down_and_modify_selection(0),
            BufferMethod::MoveLeftAndModifySelection => buffer.move_left_and_modify_selection(0),
            BufferMethod::MoveRightAndModifySelection => buffer.move_right_and_modify_selection(0),
            BufferMethod::DeleteForward => buffer.delete_forward(0),
            BufferMethod::DeleteBackward => buffer.delete_backward(0),
        }
        buffer.check_invariants(0);
    }
});
