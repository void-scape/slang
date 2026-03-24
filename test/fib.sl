extern fn printf(fmt: str, ...);
fn main() -> i32 {
    let total = 69;
    let i = 1;
    while i <= total {
        let n = i - 1;
        let a: u64 = 0;
        let b: u64 = 1;
        let c: u64 = 0;
        while n > 0 {
            c = a + b;
            a = b;
            b = c;
            n = n - 1;
        }
        printf("%d fib number: %ld\n", i, c);
        i = i + 1;
    }
    return 0;
}
