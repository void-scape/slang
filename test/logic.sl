extern fn printf(fmt: str, ...);
extern fn exit(code: u64);
fn main() -> u64 {
    if 69 == 69 && 1 != 2 {
        printf("Log1\n");
    }
    if 12 != 11 || 1 != 1 {
        printf("Log2\n");
    }
    if first() || second() || third() {
        printf("Log3\n");
    }
    if first() && (second() || third()) {
        printf("Log3\n");
    }
    if 0 + 1 && 69 {
        printf("Valid1\n");
    }
    if 1 - 1 || 0 + 0 + 1 {
        printf("Valid2\n");
    }
    if 69 != 69 {
        printf("YOU SHOULDNT SEE ME\n");
        exit(69);
    }
    if 12 == 11 {
        printf("YOU SHOULDNT SEE ME\n");
        exit(69);
    }
    return 0;
}
fn first() -> u64 {
    printf("first\n");
    return 0;
}
fn second() -> u64 {
    printf("second\n");
    return 1;
}
fn third() -> u64 {
    printf("third\n");
    return 1;
}
