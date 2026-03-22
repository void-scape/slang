fn main() {
    let mut args = std::env::args();
    let bin = args.next().unwrap();
    let path = match args.next() {
        Some(arg) => arg,
        None => {
            usage(&bin);
            std::process::exit(1);
        }
    };
    let flags = args.collect::<Vec<_>>();
    let run = flags.iter().any(|s| s == "-r");
    let capture = flags.iter().any(|s| s == "-c");
    let log = flags.iter().any(|s| s == "-l");
    let codegen = flags.iter().any(|s| s == "-g");
    if flags.iter().any(|s| s == "-h") {
        usage(&bin);
        return;
    }
    slang::compile(slang::Flags {
        log,
        run,
        capture,
        codegen,
        input: vec![path],
        output: None,
    });
}

fn usage(bin: &str) {
    println!("USAGE: {bin} input [-hrclg]");
    println!("  -h: Emit this message");
    println!("  -r: Execute the program after compilation");
    println!("  -c: Capture the stdout of the program to a file");
    println!("  -l: Log");
    println!("  -g: Output code generation");
    println!("No inputs provided");
}
