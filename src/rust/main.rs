pub mod inners;
pub mod tea;
pub mod uv;

use std::error::Error;

use crate::tea::{
    Program, ProgramContext, command,
    message::{Message, MessageType},
    model::Model,
};

#[derive(Clone)]
pub struct Frame(Vec<u8>);

impl Model for Frame {
    fn view(&self) -> Box<[u8]> {
        self.0.clone().into_boxed_slice()
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let program = Program::init(Frame(Vec::new())).unwrap();

    program.update(
        MessageType::Keypress,
        |program: &ProgramContext<Frame>, msg: &Message| {
            if let Message::Keypress(keys) = msg {
                if keys[0] == 03 {
                    Some(command::Terminate.into())
                } else {
                    for key in keys {
                        program.model.borrow_mut().0.push(*key);
                    }
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
