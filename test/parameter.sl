extern fn printf(fmt: str, ...);
fn many(
	n: u64, a: u64, b: u64, n1: u64, a1: u64, b1: u64, 
	n2: u64, a2: u64, b2: u64, n3: u64, a3: u64, b3: u64) 
	-> u64 
{
	printf("I am in a function! %d %d %d %d %d %d %d %d %d %d %d %d\n", 
		n, a, b + a1, n1 << n, a1, b1 >> b, n2 - a, a2, b2, n3, a3 + b1, b3 + n3);
	return 80082;
}
fn main() -> u64 {
	let returned = many(0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11);
	printf("The value returned! %d\n", returned);
	return 0;
}
