use std::env;

pub struct Opts {
    pub subcommand: String,
    pub path: String,
}

pub fn get_opts() -> Opts {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: rustypipe <run|validate> <pipeline.yaml>");
        std::process::exit(1);
    }
    Opts {
        subcommand: args[1].clone(),
        path: args[2].clone(),
    }
}
