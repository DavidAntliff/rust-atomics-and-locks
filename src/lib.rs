#[inline(never)]
pub fn add_ten(num: &mut i32) {
    *num += 10;
}

// $ cargo asm --lib --target=aarch64-unknown-linux-musl add_ten
//
// .section .text.atomics_and_locks::add_ten,"ax",@progbits
// 	.globl	atomics_and_locks::add_ten
// 	.p2align	2
// .type	atomics_and_locks::add_ten,@function
// atomics_and_locks::add_ten:
// 	.cfi_startproc
// 	ldr w8, [x0]
// 	add w8, w8, #10
// 	str w8, [x0]
// 	ret

// $ cargo asm --lib --target=x86_64-unknown-linux-musl add_ten
//
// .section .text.atomics_and_locks::add_ten,"ax",@progbits
// 	.globl	atomics_and_locks::add_ten
// 	.prefalign	4, .Lfunc_end0, nop
// .type	atomics_and_locks::add_ten,@function
// atomics_and_locks::add_ten:
// 	.cfi_startproc
// 	add dword ptr [rdi], 10
// 	ret
