// const PERIPH_BASE_NS: u64 = 0x40000000;

extern fn printf(fmt: str, ...);
// 
// fn passthrough() -> u64 {
// 	return 69;
// }

fn main() -> u64 {
	let a = 1;
	let b = 2;
	let c = 0;
	c = a + b;
	// let value = passthrough();
	printf("%d\n", c);
	return 0;
}
