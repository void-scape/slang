#![feature(bool_to_result)]
#![feature(uint_bit_width)]

use std::{
    io::Write,
    process::{Command, Stdio},
};

mod asm;
mod ir;
mod parse;
mod tokenize;

fn main() {
    fn arg_or_usage(bin: &str, args: &mut std::env::Args) -> String {
        match args.next() {
            Some(arg) => arg,
            None => {
                println!("USAGE: {bin} input");
                println!("No inputs provided");
                std::process::exit(1);
            }
        }
    }

    let mut args = std::env::args();
    let bin = args.next().unwrap();
    let path = arg_or_usage(&bin, &mut args);
    let input = std::fs::read_to_string(&path).unwrap().leak();

    let now = std::time::Instant::now();
    let tokens = match tokenize::tokenize(input) {
        Ok(tokens) => tokens,
        Err(err) => {
            tokenize::pretty_print(&path, input, err);
            std::process::exit(1);
        }
    };
    let tokenize_dur = now.elapsed();

    let now = std::time::Instant::now();
    let funcs = match parse::parse(&mut tokens.as_slice()) {
        Ok(funcs) => funcs,
        Err(err) => {
            parse::pretty_print(&path, input, err);
            std::process::exit(1);
        }
    };
    let parse_dur = now.elapsed();

    let now = std::time::Instant::now();
    let asm = asm::asm(&funcs);
    let asm_dur = now.elapsed();
    println!("{asm}");

    let now = std::time::Instant::now();
    assembler(&asm);
    linker();
    let link_dur = now.elapsed();

    println!("Tokenize\t{:.4}", tokenize_dur.as_secs_f32());
    println!("Parsing \t{:.4}", parse_dur.as_secs_f32());
    println!("Codegen \t{:.4}", asm_dur.as_secs_f32());
    println!("Linking \t{:.4}", link_dur.as_secs_f32());
    println!(
        "Total   \t{:.4}",
        tokenize_dur.as_secs_f32()
            + parse_dur.as_secs_f32()
            + asm_dur.as_secs_f32()
            + link_dur.as_secs_f32()
    );
}

fn assembler(asm: &str) {
    let mut ass = Command::new("as")
        .args(["-o", "/tmp/slbuild.o", "-"])
        .stdin(Stdio::piped())
        .spawn()
        .unwrap();
    ass.stdin.take().unwrap().write_all(asm.as_bytes()).unwrap();
    let output = ass.wait_with_output().unwrap();
    if !output.status.success() {
        std::process::exit(1);
    }
}

fn linker() {
    let output = std::process::Command::new("ld")
        .args([
            "-o",
            "sl",
            "-e",
            "_start",
            "-lSystem",
            "-syslibroot",
            "/Applications/Xcode.app/Contents/Developer/Platforms/\
            MacOSX.platform/Developer/SDKs/MacOSX15.5.sdk",
            "/tmp/slbuild.o",
        ])
        .spawn()
        .unwrap()
        .wait()
        .unwrap();
    if !output.success() {
        std::process::exit(1);
    }
}
