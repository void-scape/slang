extern fn printf(fmt: str, ...);
extern fn exit(code: i32);
fn assert_eq(a: u64, b: u64) {
    if a != b {
        printf("FAILED\n");
        exit(1);
    }
}
fn main() -> i32 {
    const OFFSET: u64 = BASE + 1 - 1;
    const PERIPH_BASE_NS: u64 = 0x40000000 | (0x69 << OFFSET);
    printf("0x%x\n", PERIPH_BASE_NS);
    assert_eq(0x40000690, PERIPH_BASE_NS);
    return 0;
}
const BASE: u64 = 2 * 2;
