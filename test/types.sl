extern fn exit(code: i32);
extern fn printf(fmt: str, ...);
fn true() -> i8 {
    return 1;
}
fn main() -> i32 {
    let x = 0;
    if x + 0 || (true() && 0) {
        exit(1);
    }
    return 0;
}
