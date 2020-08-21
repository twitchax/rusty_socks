
#[cfg(test)]
mod tests {
    extern crate test;
    use test::Bencher;

    pub fn add_two(a: i32) -> i32 {
        a + 2
    }

    #[test]
    fn it_works() {
        return assert_eq!(4, add_two(2));
    }

    #[bench]
    fn bench_add_two(b: &mut Bencher) {
        b.iter(|| add_two(2));
    }
}