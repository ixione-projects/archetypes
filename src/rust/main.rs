pub mod inners;
pub mod tea;
pub mod uv;

use std::{
    cell::RefCell,
    error::Error,
    io::{stdin, stdout},
    os::fd::AsRawFd,
    rc::Rc,
};

use crate::tea::{Message, MessageType, Program};

const USAGE: &str = "Usage: ./archetypes";

pub struct Frame(String);

fn main() -> Result<(), Box<dyn Error>> {
    let program = Rc::new(RefCell::new(
        Program::init(
            Frame(String::from("")),
            stdin().as_raw_fd(),
            stdout().as_raw_fd(),
        )
        .unwrap(),
    ));

    let program_clone = program.clone();
    let on_keypress = move |_: &Frame, msg: &Message| match msg {
        Message::Keypress(keys) => {
            if keys[0] == 03 {
                program_clone.borrow_mut().quit();
            }
            println!("{}", String::from_utf8(keys.clone()).unwrap())
        }
    };

    program.borrow_mut().on(MessageType::Keypress, on_keypress);

    program.borrow_mut().run()?;

    Ok(())
}
