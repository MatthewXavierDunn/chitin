use std::{iter::FilterMap, str::SplitWhitespace};

#[derive(Debug, PartialEq, Eq)]
pub enum Token<'a> {
    Text(&'a str),
    Seq,
    Redirect,
    RedirectAppend,
    RedirectInsert,
}

impl<'a> From<&'a str> for Token<'a> {
    fn from(s: &'a str) -> Self {
        match s {
            ";" => Self::Seq,
            ">" => Self::Redirect,
            ">>" => Self::RedirectAppend,
            ">+" => Self::RedirectInsert,
            _ => Self::Text(s),
        }
    }
}

impl<'a> Token<'a> {
    pub fn unwrap(self) -> &'a str {
        match self {
            Self::Text(s) => s,
            _ => panic!("tried to unwrap non-text token"),
        }
    }

    pub fn is_text(&self) -> bool {
        match self {
            Self::Text(_) => true,
            _ => false,
        }
    }
}

#[derive(Debug)]
pub struct Lexer<'a> {
    stream: FilterMap<SplitWhitespace<'a>, fn(&'a str) -> Option<&'a str>>,
    delim: Option<&'a str>,
    remain: Option<&'a str>,
}

impl<'a> Lexer<'a> {
    pub fn new(input: &'a str) -> Self {
        Self {
            stream: input.split_whitespace().filter_map(|s| match s.trim() {
                "" => None,
                s => Some(s),
            }),
            delim: None,
            remain: None,
        }
    }

    fn find(&mut self, next: &'a str, pat: &'static str) -> Option<Token<'a>> {
        Some(Token::from(match next.find(pat) {
            Some(n) => {
                let (mut next, remain) = next.split_at(n);
                let (delim, remain) = remain.split_at(pat.len());
                if next.is_empty() {
                    next = delim;
                } else {
                    self.delim = Some(delim);
                }
                if !remain.is_empty() {
                    self.remain = Some(remain);
                }
                next
            }
            None => next
        }))
    }
}


impl<'a> Iterator for Lexer<'a> {
    type Item = Token<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(next) = self.delim.take() {
            return Some(Token::from(next));
        }
        let next = match self.remain.take() {
            None => self.stream.next(),
            remain => remain,
        }?;
        self.find(next, ">>")
            .or_else(|| self.find(next, ">+"))
            .or_else(|| self.find(next, ">"))
            .or_else(|| self.find(next, ";"))
    }
}
