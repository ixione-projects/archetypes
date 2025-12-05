use std::{collections::VecDeque, fmt::Debug};

use crate::uv::Buf;

#[derive(Debug, Clone, Copy)]
pub enum KeyName {
    BEL,
    BS,
    HT,
    LF,
    VT,
    FF,
    CR,
    ESC,
    DEL,

    NONE,
}

pub struct KeyCode {
    pub key: KeyName,
    pub code: Vec<u8>,
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
}

pub struct KeyCodeParser {
    buf: VecDeque<u8>,
    incomplete: Vec<u8>,
}

pub fn is_shift(ch: u8) -> bool {
    return (ch >= b'A' && ch <= b'Z')
        || (ch >= b'\x21' && ch <= b'\x26')
        || (ch >= b'\x28' && ch <= b'\x2b')
        || (ch == b'\x3a')
        || (ch == b'\x3c')
        || (ch >= b'\x3e' && ch <= b'\x40')
        || (ch >= b'\x5e' && ch <= b'\x5f')
        || (ch >= b'\x7b' && ch <= b'\x7e');
}

// TODO: impl Iterator
impl KeyCodeParser {
    pub fn parse_keycode(&mut self) -> Option<KeyCode> {
        self.incomplete.clear();

        let ch = self.advance();
        while ch != 0 {
            match ch {
                b'\x07' => {
                    return Some(KeyCode {
                        key: KeyName::BEL,
                        code: vec![ch],
                        shift: false,
                        ctrl: true,
                        alt: false,
                    });
                }
                b'\x08' => {
                    return Some(KeyCode {
                        key: KeyName::BS,
                        code: vec![ch],
                        shift: false,
                        ctrl: true,
                        alt: false,
                    });
                }
                b'\t' => {
                    return Some(KeyCode {
                        key: KeyName::HT,
                        code: vec![ch],
                        shift: false,
                        ctrl: true,
                        alt: false,
                    });
                }
                b'\n' => {
                    return Some(KeyCode {
                        key: KeyName::LF,
                        code: vec![ch],
                        shift: false,
                        ctrl: true,
                        alt: false,
                    });
                }
                b'\x0b' => {
                    return Some(KeyCode {
                        key: KeyName::VT,
                        code: vec![ch],
                        shift: false,
                        ctrl: true,
                        alt: false,
                    });
                }
                b'\x0c' => {
                    return Some(KeyCode {
                        key: KeyName::FF,
                        code: vec![ch],
                        shift: false,
                        ctrl: true,
                        alt: false,
                    });
                }
                b'\r' => {
                    return Some(KeyCode {
                        key: KeyName::CR,
                        code: vec![ch],
                        shift: false,
                        ctrl: true,
                        alt: false,
                    });
                }
                b'\x1b' => {
                    if !self.r#match(b'[') {
                        break;
                    }
                    self.parse_num();
                    if !self.r#match(b';') {
                        break;
                    }
                    self.parse_num();
                    if !self.r#match(b'R') {
                        break;
                    }

                    let mut code = Vec::with_capacity(self.incomplete.len());
                    for ch in self.incomplete.iter() {
                        code.push(*ch);
                    }

                    return Some(KeyCode {
                        key: KeyName::NONE,
                        code: code,
                        shift: false,
                        ctrl: false,
                        alt: false,
                    });
                }
                b'\x7f' => {
                    return Some(KeyCode {
                        key: KeyName::DEL,
                        code: vec![ch],
                        shift: false,
                        ctrl: true,
                        alt: false,
                    });
                }
                _ => {
                    if ch < b'\x20' {
                        return Some(KeyCode {
                            key: KeyName::NONE,
                            code: vec![ch],
                            shift: false,
                            ctrl: true,
                            alt: false,
                        });
                    }

                    if ch >= b'\x20' && ch <= b'\x7e' {
                        return Some(KeyCode {
                            key: KeyName::NONE,
                            code: vec![ch],
                            shift: is_shift(ch),
                            ctrl: false,
                            alt: false,
                        });
                    }
                }
            }
        }

        for _ in 0..self.incomplete.len() {
            self.buf.push_front(self.incomplete.pop().unwrap());
        }
        None
    }

    fn parse_num(&mut self) {
        while let Some(ch) = self.buf.front() {
            if ch <= &b'0' || ch >= &b'9' {
                break;
            }
            self.buf.pop_front();
        }
    }

    pub fn buffer(&mut self, buf: &Buf) {
        for ch in buf.iter() {
            self.buf.push_back(*ch);
        }
    }

    fn r#match(&mut self, expect: u8) -> bool {
        if self.advance() != expect {
            false
        } else {
            true
        }
    }

    fn advance(&mut self) -> u8 {
        match self.buf.pop_front() {
            Some(ch) => {
                self.incomplete.push(ch);
                ch
            }
            None => 0,
        }
    }
}

impl Default for KeyCodeParser {
    fn default() -> Self {
        Self {
            buf: Default::default(),
            incomplete: Default::default(),
        }
    }
}

impl Debug for KeyCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KeyCode")
            .field("key", &self.key)
            .field("code", &self.code)
            .field("shift", &self.shift)
            .field("ctrl", &self.ctrl)
            .field("alt", &self.alt)
            .finish()
    }
}

impl Clone for KeyCode {
    fn clone(&self) -> Self {
        Self {
            key: self.key.clone(),
            code: self.code.clone(),
            shift: self.shift.clone(),
            ctrl: self.ctrl.clone(),
            alt: self.alt.clone(),
        }
    }
}
