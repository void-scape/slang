fn printf(fmt: u64, ...) {
	// intrinsics //
}

fn exit(code: u64) {
	// intrinsics //
}

fn main() -> u64 {
	logical();
	bits();

	printf("4 %% 2 == %d\n", 4 % 2);
	printf("3 %% 2 == %d\n", 3 % 2);
	printf("(3 + 1) %% 2 == %d\n", (3 + 1) % 2);
	printf("(2 + 1) %% 2 == %d\n", (2 + 1) % 2);

	let returned = fib(0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11);
	printf("The value returned! %d\n", returned);

	let total = 10;
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
		let format = "nth";
		printf("%d%s fib number: %d\n", i, format, c);
		i = i + 1;
	}

	return 0;
}

fn fib(
	n: u64, a: u64, b: u64, n1: u64, a1: u64, b1: u64, 
	n2: u64, a2: u64, b2: u64, n3: u64, a3: u64, b3: u64) 
	-> u64 
{
	printf("I am in a function! %d %d %d %d %d %d %d %d %d %d %d %d\n", 
		n, a, b, n1, a1, b1, n2, a2, b2, n3, a3, b3);
	return 80082;
}

fn logical() {
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

fn bits() {
	binary_op(9, 8, 9 & 8, "&");
	binary_op(9, 8, 9 | 8, "|");
	binary_op(9, 8, 9 ^ 8, "^");

	printf("!");
	printf_binary(4, 0);
	printf(" = ");
	printf_binary(4, !0);
	printf("\n");
}

fn binary_op(a: u64, b: u64, c: u64, fmt: u64) {
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
