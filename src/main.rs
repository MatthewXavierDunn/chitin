mod lexer;
mod ast;
use std::{io::{self, Write, BufRead, BufReader}, process::Command, env, fs::{File, OpenOptions}, path::PathBuf};

use ast::{Expr, Cmd, Combinator};
use lexer::Lexer;
use colored::Colorize;

pub enum ResultKind {
    Exit,
    Ok,
}

type CommandResult = Result<ResultKind, io::Error>;

pub trait Runnable {
    fn run(self, out: &mut impl Write) -> CommandResult;
}

impl<'a> Runnable for Expr<'a> {
    fn run(self, out: &mut impl Write) -> CommandResult {
        match self {
            Self::NoOp => Ok(ResultKind::Ok),
            Self::Seq(combinator, rest) => {
                match combinator.run(out)? {
                    ResultKind::Ok => rest.run(out),
                    exit => Ok(exit),
                }
            }
        }
    }
}

impl<'a> Runnable for Combinator<'a> {
    fn run(self, out: &mut impl Write) -> CommandResult {
        match self {
            Self::Identity(cmd) => cmd.run(out),
            Self::Redirect(cmd, output) => {
                let mut file = File::create(output)?;
                cmd.run(&mut file)
            }
            Self::RedirectAppend(cmd, output) => {
                let mut file = OpenOptions::new().append(true).create(true).open(output)?;
                cmd.run(&mut file)
            }
            Self::RedirectInsert(cmd, output) => {
                let mut orig = File::open(output)?;
                let dir = PathBuf::from("myshell_tmp.txt");
                let mut temp = File::create(&dir)?;

                let res = cmd.run(&mut temp);

                io::copy(&mut orig, &mut temp)?;
                std::fs::remove_file(output)?;
                std::fs::rename(&dir, &output)?;

                res
            }
        }
    }
}

impl<'a> Runnable for Cmd<'a> {
    fn run(self, out: &mut impl Write) -> CommandResult {
        match self {
            Cmd::NoOp => Ok(ResultKind::Ok),
            Cmd::Pwd => {
                let dir = env::current_dir()?;
                write!(out, "{}\n", dir.display())?;
                Ok(ResultKind::Ok)
            },
            Cmd::Cd(opt_path) => {
                if let Some(path) = opt_path {
                    env::set_current_dir(path)?;
                    Ok(ResultKind::Ok)
                } else {
                    env::set_current_dir(
                        env::var("HOME").map_err(
                            |_| io::Error::new(io::ErrorKind::Other, "could not locate home directory")
                        )?
                    )?;
                    Ok(ResultKind::Ok)
                }
            }
            Cmd::Other(cmd, args) => {
                let output = Command::new(cmd)
                    .args(args)
                    .spawn()?
                    .wait_with_output()?;
                out.write_all(output.stdout.as_slice())?;
                Ok(ResultKind::Ok)
            }
            Cmd::Exit => Ok(ResultKind::Exit),
        }
    }
}

fn interactive() -> io::Result<()> {
    let mut buffer = String::new();
    let stdin = io::stdin();
    let mut stdout = io::stdout();

    loop {
        write!(stdout, "{}", "chitin> ".bold())?;
        stdout.flush()?;

        buffer.clear();
        let n = stdin.read_line(&mut buffer)?;
        let input = buffer[..n].trim();

        match Expr::try_from(Lexer::new(input)) {
            Ok(comb) => match comb.run(&mut stdout) {
                Ok(ResultKind::Ok) => (),
                Err(reason) => {
                    write!(stdout, "{}\n", reason.to_string().bright_red())?;
                }
                Ok(ResultKind::Exit) => break,
            }
            Err(reason) => write!(stdout, "{}\n", reason.to_string().bright_red())?,
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
        write!(stdout, "{}", input.bold())?;

        match Expr::try_from(Lexer::new(input.as_str())) {
            Ok(comb) => match comb.run(&mut stdout) {
                Ok(ResultKind::Ok) => (),
                Err(reason) => {
                    write!(stdout, "{}\n", reason.to_string().bright_red().red())?;
                }
                Ok(ResultKind::Exit) => break,
            }
            Err(reason) => write!(stdout, "{}\n", reason.to_string().bright_red())?,
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
