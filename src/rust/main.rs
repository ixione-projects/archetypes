pub mod inners;
pub mod tea;
pub mod uv;

use std::error::Error;

use crate::tea::{
    Program, ProgramContext,
    message::{Message, MessageType},
};

const USAGE: &str = "Usage: ./archetypes";

#[derive(Debug, Clone, Copy)]
pub struct Frame(i32);

fn main() -> Result<(), Box<dyn Error>> {
    let program = Program::init(Frame(0)).unwrap();

    program.on(
        MessageType::Keypress,
        |_: &ProgramContext<Frame>, msg: &Message| {
            if let Message::Keypress(keys) = msg {
                if keys[0] == 03 {
                    Some(Message::Terminate)
                } else {
                    println!("{}", String::from_utf8(keys.clone()).unwrap());
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
