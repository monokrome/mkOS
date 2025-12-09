use anyhow::{Context, Result};
use std::ffi::OsStr;
use std::process::{Command, Stdio};

const CYAN: &str = "\x1b[36m";
const RESET: &str = "\x1b[0m";

pub fn run<I, S>(program: &str, args: I) -> Result<()>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let args: Vec<_> = args.into_iter().collect();
    let args_str: Vec<_> = args.iter().map(|s| s.as_ref().to_string_lossy()).collect();

    println!("{}> {} {}{}", CYAN, program, args_str.join(" "), RESET);

    let status = Command::new(program)
        .args(&args)
        .status()
        .with_context(|| format!("Failed to run {}", program))?;

    if !status.success() {
        anyhow::bail!("{} failed with exit code {:?}", program, status.code());
    }

    Ok(())
}

pub fn run_with_stdin<I, S>(program: &str, args: I, input: &[u8]) -> Result<()>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    use std::io::Write;

    let args: Vec<_> = args.into_iter().collect();
    let args_str: Vec<_> = args.iter().map(|s| s.as_ref().to_string_lossy()).collect();

    println!("{}> {} {}{}", CYAN, program, args_str.join(" "), RESET);

    let mut child = Command::new(program)
        .args(&args)
        .stdin(Stdio::piped())
        .spawn()
        .with_context(|| format!("Failed to run {}", program))?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(input)?;
    }

    let status = child.wait()?;
    if !status.success() {
        anyhow::bail!("{} failed with exit code {:?}", program, status.code());
    }

    Ok(())
}

pub fn run_output<I, S>(program: &str, args: I) -> Result<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let args: Vec<_> = args.into_iter().collect();
    let args_str: Vec<_> = args.iter().map(|s| s.as_ref().to_string_lossy()).collect();

    println!("{}> {} {}{}", CYAN, program, args_str.join(" "), RESET);

    let output = Command::new(program)
        .args(&args)
        .output()
        .with_context(|| format!("Failed to run {}", program))?;

    if !output.status.success() {
        anyhow::bail!(
            "{} failed with exit code {:?}",
            program,
            output.status.code()
        );
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}
