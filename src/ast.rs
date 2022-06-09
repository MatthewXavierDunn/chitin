use std::iter::Peekable;

use crate::lexer::{Token, Lexer};

type LexInput<'a> = Peekable<Lexer<'a>>;
type LexOutput<'a, T> = Result<(T, Peekable<Lexer<'a>>), &'static str>;

pub trait FromLexer<'a>: Sized {
    fn from_lexer(lexer: LexInput<'a>) -> LexOutput<'a, Self>;
}

// Expr := MultiCmd
// MultiCmd := Cmd ; MultiCmd | e
// Cmd := Once | Redirect | 
#[derive(Debug)]
pub enum Expr<'a> {
    NoOp,
    Seq(Combinator<'a>, Box<Expr<'a>>),
}

impl<'a> FromLexer<'a> for Expr<'a> {
    fn from_lexer(mut lexer: LexInput<'a>) -> LexOutput<'a, Self> {
        if lexer.peek().is_none() {
            return Ok((Self::NoOp, lexer));
        }
        let combinator;
        (combinator, lexer) = Combinator::from_lexer(lexer)?;
        let rest;
        if lexer.next_if_eq(&Token::Op(";")).is_some() {
            (rest, lexer) = Self::from_lexer(lexer)?
        } else {
            rest = Self::NoOp
        };
        Ok((Self::Seq(combinator, Box::new(rest)), lexer))
    }
}

impl<'a> TryFrom<Lexer<'a>> for Expr<'a> {
    type Error = &'static str;

    fn try_from(lexer: Lexer<'a>) -> Result<Self, Self::Error> {
        Self::from_lexer(lexer.peekable()).map(|(exp, _)| exp)
    }
}

#[derive(Debug)]
pub enum Combinator<'a> {
    Identity(Cmd<'a>),
    Redirect(Cmd<'a>, &'a str),
    RedirectAppend(Cmd<'a>, &'a str),
    RedirectInsert(Cmd<'a>, &'a str),
}

impl<'a> FromLexer<'a> for Combinator<'a> {
    fn from_lexer(mut lexer: LexInput<'a>) -> LexOutput<'a, Self> {
        let cmd;
        (cmd, lexer) = Cmd::from_lexer(lexer)?;
        Ok((if let Some(op) = lexer.next_if(|t| match t {
            Token::Op(">") => true,
            Token::Op(">>") => true,
            Token::Op(">+") => true,
            _ => false,
        }) {
            let out = lexer.next_if(Token::is_arg).ok_or("expected argument")?.unwrap();
            match *op {
                ">" => Self::Redirect(cmd, out),
                ">>" => Self::RedirectAppend(cmd, out),
                ">+" => Self::RedirectInsert(cmd, out),
                _ => panic!("unexpected operator"),
            }
        } else {
            Self::Identity(cmd)
        }, lexer))
    }
}

#[derive(Debug)]
pub enum Cmd<'a> {
    Exit,
    Cd(Option<&'a str>),
    Pwd,
    Other(&'a str, Vec<&'a str>),
    NoOp,
}

impl<'a> FromLexer<'a> for Cmd<'a> {
    fn from_lexer(mut lexer: LexInput<'a>) -> LexOutput<'a, Self> {
        match lexer.next() {
            Some(Token::Arg(cmd)) => {
                let mut args = Vec::new();
                while let Some(arg) = lexer.next_if(Token::is_arg) {
                    args.push(*arg);
                }
                Self::try_from((cmd, args)).map(|cmd| (cmd, lexer))
            }
            None => Ok((Self::NoOp, lexer)),
            _ => Err("expected argument or empty line"),
        }
    }
}

impl<'a> TryFrom<(&'a str, Vec<&'a str>)> for Cmd<'a> {
    type Error = &'static str;

    fn try_from((cmd, args): (&'a str, Vec<&'a str>)) -> Result<Self, Self::Error> {
        match cmd {
            "exit" =>
                if args.len() == 0 {
                    Ok(Self::Exit)
                } else {
                    Err("wrong number of arguments supplied to 'exit'")
                }
            "cd" =>
                match args.len() {
                    0 => Ok(Self::Cd(None)),
                    1 => Ok(Self::Cd(Some(args[0]))),
                    _ => Err("wrong number of arguments supplied to 'cd'"),
                }
            "pwd" =>
                if args.len() == 0 {
                    Ok(Self::Pwd)
                } else {
                    Err("wrong number of arguments supplied to 'pwd'")
                }
            _ => Ok(Self::Other(cmd, args))
        }
    }
}
