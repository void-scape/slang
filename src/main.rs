use std::io::IsTerminal;

fn main() {
    let mut args = std::env::args();
    let bin = args.next().unwrap();
    let path = match args.next() {
        Some(arg) => arg,
        None => {
            println!("USAGE: {bin} input");
            println!("No inputs provided");
            std::process::exit(1);
        }
    };
    let flags = args.collect::<Vec<_>>();
    let run = flags.iter().any(|s| s == "-r");
    let capture = flags.iter().any(|s| s == "-c");
    let asm = flags.iter().any(|s| s == "-s");
    if let Some(report) = slang::compile(&path) {
        if asm {
            println!("{}", report.asm);
        }

        if capture {
            let path = format!("{}.txt", path.strip_suffix(".sl").unwrap());
            let result = std::process::Command::new("/tmp/slexec").output().unwrap();
            std::fs::write(&path, result.stdout).unwrap();
            println!("Captured stdout to {}", path);
        }

        if std::io::stdout().is_terminal() {
            println!("\x1b[92m\x1b[1mCompiled\x1b[0m `{path}`");
            println!("\x1b[38;5;246m... Parsing \t{:.4}\x1b[0m", report.parsing);
            println!("\x1b[38;5;246m... Codegen \t{:.4}\x1b[0m", report.codegen);
            println!("\x1b[38;5;246m... Linking \t{:.4}\x1b[0m", report.linking);
            println!(
                "\x1b[38;5;246m... Total   \t{:.4}\x1b[0m",
                report.parsing + report.codegen + report.linking
            );
        } else {
            println!("Compiled `{path}`");
            println!("... Parsing \t{:.4}", report.parsing);
            println!("... Codegen \t{:.4}", report.codegen);
            println!("... Linking \t{:.4}", report.linking);
            println!(
                "... Total   \t{:.4}",
                report.parsing + report.codegen + report.linking
            );
        }

        if run {
            let result = std::process::Command::new("/tmp/slexec").status().unwrap();
            std::process::exit(result.code().unwrap_or(0));
        }
    }
}
