use std::ops::{Deref, DerefMut};


const RESERVED_OP: &[&'static str] = &[
    ";",
    ">>",
    ">",
    ">+",
    "|",
];

#[derive(Debug, PartialEq, Eq)]
pub enum Token<'a> {
    Arg(&'a str),
    Op(&'a str),
}

impl<'a> Token<'a> {
    pub fn is_arg(&self) -> bool {
        match self {
            Self::Arg(_) => true,
            _ => false,
        }
    }

    pub fn is_op(&self) -> bool {
        match self {
            Self::Op(_) => true,
            _ => false,
        }
    }

    pub fn unwrap(self) -> &'a str {
        *self
    }
}

impl<'a> Deref for Token<'a> {
    type Target = &'a str;

    fn deref(&self) -> &Self::Target {
        match self {
            Self::Arg(s) => s,
            Self::Op(s) => s,
        }
    }
}

impl<'a> DerefMut for Token<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        match self {
            Self::Arg(s) => s,
            Self::Op(s) => s,
        }
    }
}

#[derive(Debug)]
pub struct Lexer<'a> {
    input: &'a str,
    delim: Option<Token<'a>>,
}

impl<'a> Lexer<'a> {
    pub fn new(input: &'a str) -> Self {
        Self {
            input,
            delim: None,
        }
    }
}

impl<'a> Iterator for Lexer<'a> {
    type Item = Token<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(token) = self.delim.take() {
            Some(token)
        } else if self.input.is_empty() {
            None
        } else {
            let mut found = None;
            'chars: for i in 0..self.input.len() {
                match self.input.get(i..) {
                    Some(s) if s.starts_with(|c: char| c.is_whitespace()) => {
                        found = Some((i, None));
                        break;
                    }
                    _ => (),
                }
                for &op in RESERVED_OP {
                    match self.input.get(i..i + op.len()) {
                        Some(slice) if slice == op => {
                            found = Some((i, Some(slice)));
                            break 'chars;
                        }
                        _ => (),
                    }
                }
            }
            if let Some((i, tok_typ)) = found {
                let mut slice;
                (slice, self.input) = self.input.split_at(i);
                if let Some(tok) = tok_typ {
                    // delim
                    if slice.is_empty() {
                        // delim at start
                        (slice, self.input) = self.input.split_at(tok.len());
                        Some(Token::Op(slice))
                    } else {
                        // delim somewhere
                        let delim_slice;
                        (delim_slice, self.input) = self.input.split_at(tok.len());
                        self.delim = Some(Token::Op(delim_slice));
                        Some(Token::Arg(slice))
                    }
                } else {
                    // whitespace
                    if let Some((i, _)) = self.input.char_indices().find(|(_, c)| !c.is_whitespace()) {
                        (_, self.input) = self.input.split_at(i);
                    } else {
                        self.input = "";
                    }
                    if slice.is_empty() {
                        // whitespace at start
                        self.next()
                    } else {
                        Some(Token::Arg(slice))
                    }
                }
            } else {
                // nothing found
                let slice = self.input;
                self.input = "";
                Some(Token::Arg(slice))
            }
        }
    }
}
