#![feature(bool_to_result)]
#![feature(uint_bit_width)]

use std::{
    io::Write,
    process::{Command, Stdio},
};

mod arena;
mod asm;
mod ast;
mod ir;
mod parse;
mod tokenize;

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
            "/tmp/slexec",
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

pub struct Report {
    pub asm: String,
    pub parsing: f32,
    pub codegen: f32,
    pub linking: f32,
}

pub fn compile(path: &str) -> Option<Report> {
    let input = std::fs::read_to_string(path).unwrap().leak();

    let now = std::time::Instant::now();
    let tokens = match tokenize::tokenize(input) {
        Ok(tokens) => tokens,
        Err(err) => {
            tokenize::pretty_print(path, input, err);
            return None;
        }
    };
    let tokenize_dur = now.elapsed();

    let now = std::time::Instant::now();
    let tree = match parse::parse(&mut tokens.as_slice()) {
        Ok(funcs) => funcs,
        Err(err) => {
            parse::pretty_print(path, input, err);
            return None;
        }
    };
    let parse_dur = now.elapsed();

    let now = std::time::Instant::now();
    let ir = match ir::ir(&tree) {
        Ok(ir) => ir,
        Err(err) => {
            ir::pretty_print(path, input, err);
            return None;
        }
    };
    let ir_dur = now.elapsed();

    let now = std::time::Instant::now();
    let asm = asm::asm(&ir);
    let asm_dur = now.elapsed();

    let now = std::time::Instant::now();
    assembler(&asm);
    linker();
    let link_dur = now.elapsed();

    Some(Report {
        asm,
        parsing: tokenize_dur.as_secs_f32() + parse_dur.as_secs_f32(),
        codegen: ir_dur.as_secs_f32() + asm_dur.as_secs_f32(),
        linking: link_dur.as_secs_f32(),
    })
}

#[cfg(test)]
mod test {
    include!(concat!(env!("OUT_DIR"), "/slang_tests.rs"));
    fn compile_and_run(path: &str, capture: &str) {
        let capture = std::fs::read(capture).ok();
        assert!(crate::compile(path).is_some());
        let result = std::process::Command::new("/tmp/slexec").output().unwrap();
        println!("{}", String::try_from(result.stdout.clone()).unwrap());
        assert_eq!(result.status.code(), Some(0));
        if let Some(capture) = capture
            && capture != result.stdout
        {
            panic!("stdout differs");
        }
    }
}
