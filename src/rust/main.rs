pub mod inners;
pub mod tea;
pub mod uv;

use crate::tea::{Message, MessageType, Model, Program, ProgramContext, ProgramError, command};

#[derive(Clone)]
pub struct Frame(Vec<u8>);

impl Model for Frame {
    fn view(&self) -> Box<[u8]> {
        self.0.clone().into_boxed_slice()
    }
}

fn main() -> Result<(), ProgramError> {
    let mut program = Program::init(Frame(Vec::new())).unwrap();

    program.update(
        MessageType::Keypress,
        |model: &mut Frame, _: &ProgramContext, msg: &Message| {
            if let Message::Keypress(keycode) = msg {
                if keycode.code[0] == 03 {
                    Some(command::Terminate.into())
                } else {
                    model.0.extend(&keycode.code);
                    None
                }
            } else {
                None
            }
        },
    );

    program.run()?;

    Ok(())
}
