mod id_pool {

    pub trait NonUnsignedFloat {
        fn min_value() -> Self;
        fn max_value() -> Self;
    }
    impl NonUnsignedFloat for u8 {
        fn min_value() -> Self {
            u8::MIN
        }

        fn max_value() -> Self {
            u8::MAX
        }
    }
    impl NonUnsignedFloat for u16 {
        fn min_value() -> Self {
            u16::MIN
        }

        fn max_value() -> Self {
            u16::MAX
        }
    }
    impl NonUnsignedFloat for u32 {
        fn min_value() -> Self {
            u32::MIN
        }

        fn max_value() -> Self {
            u32::MAX
        }
    }
    impl NonUnsignedFloat for u64 {
        fn min_value() -> Self {
            u64::MIN
        }

        fn max_value() -> Self {
            u64::MAX
        }
    }
    impl NonUnsignedFloat for u128 {
        fn min_value() -> Self {
            u128::MIN
        }

        fn max_value() -> Self {
            u128::MAX
        }
    }
    impl NonUnsignedFloat for usize {
        fn min_value() -> Self {
            usize::MIN
        }

        fn max_value() -> Self {
            usize::MAX
        }
    }
    pub struct IdPool<T>
    where
        T: NonUnsignedFloat,
    {
        pool: Vec<T>,
    }

    impl<T> IdPool<T>
    where
        T: NonUnsignedFloat,
    {
        pub fn new(count: T) -> Self {
            let pool: Vec<T> = Vec::new();
            for i in (0..count as u128) {
                pool.push(i);
            }
            Ok(
                Self{
                    pool
                }
            )
        }
    }

    // impl Clone for
}
