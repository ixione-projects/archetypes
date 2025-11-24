pub mod inners;
pub mod tea;
pub mod uv;

use std::{
    error::Error,
    io::{stdin, stdout},
    os::fd::AsRawFd,
};

use crate::tea::{Command, Message, MessageType, Program};

const USAGE: &str = "Usage: ./archetypes";

pub struct Frame(i32);

fn main() -> Result<(), Box<dyn Error>> {
    let program = Program::init(Frame(0), stdin().as_raw_fd(), stdout().as_raw_fd()).unwrap();

    program.on(
        MessageType::Keypress,
        |_: &Frame, msg: &Message| match msg {
            Message::Keypress(keys) => {
                if keys[0] == 03 {
                    Some(Command::Quit)
                } else {
                    println!("{}", String::from_utf8(keys.clone()).unwrap());
                    None
                }
            }
        },
    );

    program.run()?;

    Ok(())
}
