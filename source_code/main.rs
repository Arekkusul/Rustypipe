use std::env;
use std::fs::File;
use std::io::{self, BufRead, BufReader};
use std::process;

fn grep(keyword: &str, input: Vec<String>) -> Vec<String> {
    input.into_iter()
        .filter(|line| line.contains(keyword))
        .collect()
}

fn uppercase(input: Vec<String>) -> Vec<String> {
    input.into_iter()
        .map(|line| line.to_uppercase())
        .collect()
}

fn sort_lines(mut input: Vec<String>) -> Vec<String> {
    input.sort();
    input
}

fn read_lines_from_stdin() -> Vec<String> {
    let stdin = io::stdin();
    stdin.lock().lines().filter_map(Result::ok).collect()
}

fn read_lines_from_file(path: &str) -> Vec<String> {
    let file = File::open(path).unwrap_or_else(|err| {
        eprintln!("❌ Failed to open file '{}': {}", path, err);
        process::exit(1);
    });
    BufReader::new(file).lines().filter_map(Result::ok).collect()
}

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage: {} [file] [--grep keyword] [--uppercase] [--sort]", args[0]);
        process::exit(1);
    }

    let mut input_file = None;
    let mut keyword = None;
    let mut do_uppercase = false;
    let mut do_sort = false;

    // Parse args
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--grep" => {
                i += 1;
                if i < args.len() {
                    keyword = Some(args[i].clone());
                }
            }
            "--uppercase" => do_uppercase = true,
            "--sort" => do_sort = true,
            arg if !arg.starts_with("--") && input_file.is_none() => {
                input_file = Some(arg.to_string());
            }
            _ => {}
        }
        i += 1;
    }

    // Read input
    let mut lines = if let Some(file) = input_file {
        read_lines_from_file(&file)
    } else {
        println!("📥 Reading from stdin (Ctrl+D to finish)...");
        read_lines_from_stdin()
    };

    // Process pipeline
    if let Some(k) = keyword {
        lines = grep(&k, lines);
    }
    if do_uppercase {
        lines = uppercase(lines);
    }
    if do_sort {
        lines = sort_lines(lines);
    }

    // Output
    for line in lines {
        println!("{}", line);
    }
}
