use core::ops::Range;

pub trait Bits {
    const LENGTH: usize;

    fn read_bits(&self, range: Range<usize>) -> Self;
    fn read_bit(&self, index: usize) -> bool;
    fn write_bit(&mut self, index: usize, value: bool);
    fn write_bits(&mut self, range: Range<usize>, value: Self);
}


#[macro_export]
macro_rules! bits_number_impl {
    ($($t:ty)*) => ($(
        impl Bits for $t {
            const LENGTH: usize = core::mem::size_of::<Self>() as usize * 8;

            fn read_bits(&self, range: Range<usize>) -> Self {
                let mask = *self << (Self::LENGTH - range.end) >> (Self::LENGTH - range.end);
                mask >> range.start
            }

            fn read_bit(&self, index: usize) -> bool {
                self & (1 << index) != 0
            }

            fn write_bit(&mut self, index: usize, value: bool) {
                if value {
                    *self |= 1 << index;
                } else {
                    *self &= !(1 << index);
                }
            }

            fn write_bits(&mut self, range: Range<usize>, value: Self) {
                let mask = !(!0 << (Self::LENGTH - range.end) >>
                (Self::LENGTH - range.end) >>
                range.start << range.start);
                *self = (*self & mask) | (value << range.start);
            }
        }
    )*)
}

bits_number_impl! {u8 u16 i32 u32 u64}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_bits() {
        let value = 0b1010u32;
        assert_eq!(value.read_bits(0..1), 0b0);
        assert_eq!(value.read_bits(1..2), 0b1);
        assert_eq!(value.read_bits(0..3), 0b010);

        let value = 0b1010_0000u8;
        assert_eq!(value.read_bits(0..5), 0b0);
        assert_eq!(value.read_bits(4..7), 0b010);
        assert_eq!(value.read_bits(7..8), 0b1);

        let value = 0x10400;
        assert_eq!(value.read_bits(16..32), 0x1);
        assert_eq!(value.read_bits(8..16), 0x4);
        assert_eq!(value.read_bits(0..8), 0x0);

        let capabilities = 0x4018200F0107FFu64;
        assert_eq!(capabilities.read_bits(0..15), 2047);
    }

    #[test]
    fn test_read_bit() {
        assert_eq!(0x4018200F0107FFu64.read_bit(16), true);
        assert_eq!(0x4018200F0107FFu64.read_bit(0), true);

        let value = 0b1010_0000u8;
        assert_eq!(value.read_bit(0), false);
        assert_eq!(value.read_bit(1), false);
        assert_eq!(value.read_bit(6), false);
        assert_eq!(value.read_bit(7), true);
    }

    #[test]
    fn test_write_bit() {
        let mut value = 0u8;
        value.write_bit(0, true);
        assert_eq!(value, 1);
        value.write_bit(1, true);
        assert_eq!(value, 3);
        value.write_bit(0, false);
        assert_eq!(value, 2);
        value.write_bit(1, false);
        assert_eq!(value, 0);
    }

    #[test]
    fn test_write_bits() {
        let mut value = 0u8;
        value.write_bits(0..2, 0b10);
        assert_eq!(value, 0b10);

        let mut value = 0u32;
        value.write_bits(0..2, 0b11);
        assert_eq!(value, 0b11);

        value.write_bits(2..4, 0b11);
        assert_eq!(value, 0b1111);

        value.write_bits(0..4, 0b1010);
        assert_eq!(value, 0b1010);
    }
}
