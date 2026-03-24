extern fn printf(fmt: str, ...);
extern fn exit(code: u64);
fn assert_eq(a: u64, b: u64) {
    if a != b {
        printf("FAILED\n");
        exit(1);
    }
}
fn main() -> u64 {
    printf("%d\n", 0xFE);
    printf("%d\n", 0b11111110);
    assert_eq(0xfe, 0b11111110);
    assert_eq(0xFE, 0xFE);
    assert_eq(0xfe, (0b1111 << 4) + 0xe);
    assert_eq(0x99, (0x9 << 4) | 0x9);
    // NOTE: All of these operations will be folded at compile time if they
    // are entirely composed of literals...
    let a: u64 = 0x4;
    let b: u64 = (a << 4);
    let c: u64 = (b >> 4);
    let d: u64 = (0x9 << 4);
    let e: u64 = d | c;
    assert_eq(0x94, e);
    printf("%d\n", 0x94);
    printf("%d\n", (0x9 << 4) | ((0x4 << 4) >> 4));
    assert_eq(0xa0, 0xf0 & 0xa0);
    printf("%d\n", (0x9 << 4) | ((0x4 << 4) >> 4));
    more_bits();
    return 0;
}
fn more_bits() {
    binary_op(9, 8, 9 & 8, "&");
    binary_op(9, 8, 9 | 8, "|");
    binary_op(9, 8, 9 ^ 8, "^");

    printf("!");
    printf_binary(4, 0);
    printf(" = ");
    printf_binary(4, !0);
    printf("\n");
}
fn binary_op(a: u64, b: u64, c: u64, fmt: str) {
    printf_binary(4, a);
    printf(" %s ", fmt);
    printf_binary(4, b);
    printf(" = ");
    printf_binary(4, c);
    printf("\n");
}
fn printf_binary(b: u64, n: u64) {
    if b > 1 {
        printf_binary(b - 1, n >> 1);
    }
    printf("%d", n & 1);
}
