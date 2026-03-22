extern fn printf(fmt: str, ...);
fn main() -> u64 {
	let total = 69;
	let i = 1;
	while i <= total {
		let n = i - 1;
		let a = 0;
		let b = 1;
		let c = 0;
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
