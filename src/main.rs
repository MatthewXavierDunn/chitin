mod lexer;
mod ast;
use std::{io::{self, Write, BufRead, BufReader}, process::Command, env, fs::{File, OpenOptions}, path::PathBuf};

use ast::{Expr, Cmd, Combinator};
use lexer::Lexer;
use colored::Colorize;

pub enum CommandResultVariants {
    Exit,
    Ok,
}

type CommandResult = Result<CommandResultVariants, io::Error>;

pub trait Runnable {
    fn run(self, out: &mut impl Write) -> CommandResult;
}

impl<'a> Runnable for Expr<'a> {
    fn run(self, out: &mut impl Write) -> CommandResult {
        match self {
            Self::NoOp => Ok(CommandResultVariants::Ok),
            Self::Seq(combinator, rest) => {
                match combinator.run(out)? {
                    CommandResultVariants::Ok => rest.run(out),
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
            Cmd::NoOp => Ok(CommandResultVariants::Ok),
            Cmd::Pwd => {
                let dir = env::current_dir()?;
                write!(out, "{}\n", dir.display())?;
                Ok(CommandResultVariants::Ok)
            },
            Cmd::Cd(path) => {
                env::set_current_dir(path)?;
                Ok(CommandResultVariants::Ok)
            }
            Cmd::Other(cmd, args) => {
                let output = Command::new(cmd)
                    .args(args)
                    .spawn()?
                    .wait_with_output()?;
                out.write_all(output.stdout.as_slice())?;
                Ok(CommandResultVariants::Ok)
            }
            Cmd::Exit => Ok(CommandResultVariants::Exit),
        }
    }
}

fn interactive() -> io::Result<()> {
    let mut buffer = String::new();
    let stdin = io::stdin();
    let mut stdout = io::stdout();

    loop {
        stdout.write_all(format!("{}", "myshell> ".red()).as_bytes())?;
        stdout.flush()?;

        buffer.clear();
        let n = stdin.read_line(&mut buffer)?;
        let input = buffer[..n].trim();

        match Expr::try_from(Lexer::new(input)) {
            Ok(comb) => match comb.run(&mut stdout) {
                Ok(CommandResultVariants::Ok) => (),
                Err(reason) => {
                    write!(stdout, "{reason}\n")?;
                }
                Ok(CommandResultVariants::Exit) => break,
            }
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

        match Expr::try_from(Lexer::new(input.as_str())) {
            Ok(comb) => match comb.run(&mut stdout) {
                Ok(CommandResultVariants::Ok) => (),
                Err(reason) => {
                    write!(stdout, "{reason}\n")?;
                }
                Ok(CommandResultVariants::Exit) => break,
            }
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
