fn main() {
    let mut args = std::env::args();
    let bin = args.next().unwrap();
    let path = match args.next() {
        Some(arg) => arg,
        None => {
            println!("USAGE: {bin} input [-rc]");
            println!("No inputs provided");
            std::process::exit(1);
        }
    };
    let flags = args.collect::<Vec<_>>();
    let run = flags.iter().any(|s| s == "-r");
    let capture = flags.iter().any(|s| s == "-c");
    slang::compile(
        vec![path],
        slang::Config {
            log: true,
            run,
            capture,
        },
    );
}
