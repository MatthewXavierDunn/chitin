mod lexer;
use std::{io::{self, Write, BufRead, BufReader}, process::Command, iter::Peekable, env, fs::{File, OpenOptions}, path::PathBuf};
use lexer::{Lexer, Token};

enum CommandResult {
    Exit,
    Ok,
    Err(io::Error),
}

impl CommandResult {
    fn and_then(self, f: impl FnOnce() -> Self) -> Self {
        match self {
            Self::Ok => f(),
            otherwise => otherwise,
        }
    }

    fn from_io_result<T>(res: Result<T, io::Error>, f: impl FnOnce(T) -> Self) -> Self {
        match res {
            Ok(v) => f(v),
            Err(err) => Self::Err(err),
        }
    }

    fn ok_from_io<T>(res: Result<T, io::Error>) -> Self {
        match res {
            Ok(_) => Self::Ok,
            Err(err) => Self::Err(err),
        }
    }
}

// Comb := Cmd | Cmd ; Comb | Comb > File | Comb >> File| Comb >+ File
// Cmd  := Path Args | e
// Args := Arg Args | e
// Arg  := -Ident | --Ident | Path | Literal
#[derive(Debug)]
enum Combinator<'a> {
    Once(Cmd<'a>),
    Seq(Cmd<'a>, Box<Combinator<'a>>),
    Redirect(Box<Combinator<'a>>, &'a str),
    RedirectAppend(Box<Combinator<'a>>, &'a str),
    RedirectInsert(Box<Combinator<'a>>, &'a str),
}

impl<'a> Combinator<'a> {
    fn parse(tokens: impl Iterator<Item = Token<'a>>) -> Result<Self, String> {
        Self::parse_peekable(&mut tokens.peekable())
    }

    fn parse_peekable(tokens: &mut Peekable<impl Iterator<Item = Token<'a>>>) -> Result<Self, String> {
        let cmd = Cmd::parse(tokens)?;
        let comb = if let Some(_) = tokens.next_if_eq(&Token::Seq) {
            let rest = Self::parse_peekable(tokens)?;
            Self::Seq(cmd, Box::new(rest))
        } else {
            Self::Once(cmd)
        };
        Ok(if let Some(_) = tokens.next_if_eq(&Token::Redirect) {
            let file = tokens.next().expect("redirect must specify output file").unwrap();
            Self::Redirect(Box::new(comb), file)
        } else if let Some(_) = tokens.next_if_eq(&Token::RedirectAppend) {
            let file = tokens.next().expect("redirect must specify output file").unwrap();
            Self::RedirectAppend(Box::new(comb), file)
        } else if let Some(_) = tokens.next_if_eq(&Token::RedirectInsert) {
            let file = tokens.next().expect("redirect must specify output file").unwrap();
            Self::RedirectInsert(Box::new(comb), file)
        } else {
            comb
        })
    }

    fn run(self, out: &mut impl Write) -> CommandResult {
        match self {
            Self::Once(cmd) => cmd.run(out),
            Self::Seq(cmd, rest) => {
                cmd.run(out).and_then(|| rest.run(out))
            }
            Self::Redirect(cmd, output) => {
                CommandResult::from_io_result(
                    File::create(output),
                    |mut file| cmd.run(&mut file)
                )
            }
            Self::RedirectAppend(cmd, output) => {
                CommandResult::from_io_result(
                    OpenOptions::new().append(true).create(true).open(output),
                    |mut file| cmd.run(&mut file)
                )
            }
            Self::RedirectInsert(cmd, output) => {
                CommandResult::from_io_result(File::open(output), |mut orig| {
                    let dir = PathBuf::from("myshell_tmp.txt");
                    CommandResult::from_io_result(File::create(&dir), |mut temp| {
                        cmd.run(&mut temp).and_then(|| {
                            CommandResult::ok_from_io(
                                io::copy(&mut orig, &mut temp)
                            ).and_then(|| CommandResult::ok_from_io(
                                std::fs::remove_file(output)
                            )).and_then(|| CommandResult::ok_from_io(
                                std::fs::rename(&dir, &output)
                            ))
                        })
                    })
                })
            }
        }
    }
}

#[derive(Debug)]
enum Cmd<'a> {
    Exit,
    Cd(&'a str),
    Pwd,
    NoOp,
    Other(&'a str, Vec<&'a str>),
}

impl<'a> Cmd<'a> {

    fn args(tokens: &mut Peekable<impl Iterator<Item = Token<'a>>>) -> Vec<&'a str> {
        let mut args = Vec::new();
        loop {
            if let Some(token) = tokens.next_if(|t| t.is_text()) {
                args.push(token.unwrap());
            } else {
                break;
            }
        }
        args
    }

    fn parse(tokens: &mut Peekable<impl Iterator<Item = Token<'a>>>) -> Result<Self, String> {
        if let Some(_) = tokens.next_if_eq(&Token::Text("exit")) {
            Self::args(tokens);
            Ok(Self::Exit)
        } else if let Some(_) = tokens.next_if_eq(&Token::Text("cd")) {
            let path = tokens.next().ok_or_else(|| String::from("cd must take at least one argument"))?;
            Self::args(tokens);
            Ok(Self::Cd(path.unwrap()))
        } else if let Some(_) = tokens.next_if_eq(&Token::Text("pwd")) {
            Self::args(tokens);
            Ok(Self::Pwd)
        } else {
            Ok(match tokens.next() {
                Some(cmd) => Self::Other(
                    cmd.unwrap(),
                    Self::args(tokens),
                ),
                None => Self::NoOp,
            })
        }
    }

    fn run(self, out: &mut impl Write) -> CommandResult {
        match self {
            Cmd::NoOp => CommandResult::Ok,
            Cmd::Pwd => CommandResult::from_io_result(env::current_dir(), |dir| {
                write!(out, "{}\n", dir.display()).unwrap();
                CommandResult::Ok
            }),
            Cmd::Cd(path) => CommandResult::ok_from_io(env::set_current_dir(path)),
            Cmd::Other(cmd, args) => {
                CommandResult::from_io_result(Command::new(cmd).args(args).spawn(), |child| {
                    CommandResult::from_io_result(
                        child.wait_with_output(),
                        |output| CommandResult::ok_from_io(out.write_all(output.stdout.as_slice()))
                    )
                })
            }
            Cmd::Exit => CommandResult::Exit,
        }
    }
}

fn interactive() -> io::Result<()> {
    let mut buffer = String::new();
    let stdin = io::stdin();
    let mut stdout = io::stdout();

    loop {
        stdout.write_all(b"myshell> ")?;
        stdout.flush()?;

        buffer.clear();
        let n = stdin.read_line(&mut buffer)?;
        let input = buffer[..n].trim();

        let tokens = Lexer::new(input);

        match Combinator::parse(tokens) {
            Ok(comb) => match comb.run(&mut stdout) {
                CommandResult::Ok => (),
                CommandResult::Err(reason) => return Err(reason),
                CommandResult::Exit => break,
            },
            Err(reason) => write!(stdout, "{reason}\n")?,
        }
    }
    Ok(())
}

fn batch(src: &String) -> io::Result<()> {
    let file = File::open(src)?;
    let reader = BufReader::new(file);
    let mut stdout = io::stdout();
    for line in reader.lines() {
        let input = line?;
        if input.is_empty() {
            continue;
        }
        write!(stdout, "{input}")?;
        let tokens = Lexer::new(input.as_str());

        match Combinator::parse(tokens) {
            Ok(comb) => match comb.run(&mut stdout) {
                CommandResult::Ok => (),
                CommandResult::Err(reason) => return Err(reason),
                CommandResult::Exit => break,
            },
            Err(reason) => write!(stdout, "{reason}\n")?,
        }
    }
    Ok(())
}

fn main() -> io::Result<()> {
    let args: Vec<_> = env::args().collect();
    match args.len() {
        1 => interactive(),
        2 => batch(&args[1]),
        _ => panic!("too many arguments")
    }
}
