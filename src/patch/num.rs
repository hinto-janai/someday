use crate::Apply;

macro_rules! impl_num {
	($($num:ty),* $(,)?) => {$( paste::paste! {
		#[doc = "Common operations for [`" $num "`]"]
		///
		/// All of these patches are assignment operations.
		///
		#[doc = "They will modify the target [`" $num "`] in place."]
		#[non_exhaustive]
		#[derive(Clone, PartialEq, PartialOrd, Eq, Ord, Debug, Hash)]
		#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
		#[cfg_attr(feature = "bincode", derive(bincode::Encode, bincode::Decode))]
		pub enum [<Patch $num:camel>] {
			/// Adds using the `+` operator
			Add($num),
			/// Substracts using the `-` operator
			Sub($num),
			/// Divides using the `/` operator
			Div($num),
			/// Multiplies using the `*` operator
			Mul($num),
			/// Modulo using the `%` operator
			Mod($num),
			/// Raises `self` to the power of this value, using exponentiation by squaring.
			/// Calls `pow()` internally.
			Pow(u32),
			/// Adds using `saturating_add()`
			SaturatingAdd($num),
			/// Substracts using `saturating_sub()`
			SaturatingSub($num),
			/// Divides using `saturating_div()`
			SaturatingDiv($num),
			/// Multiplies using `saturating_mul()`
			SaturatingMul($num),
			/// Saturating integer exponentiation.
			/// Computes self.pow(exp), saturating at the numeric bounds instead of overflowing.
			SaturatingPow(u32),
			/// Adds using `wrapping_add()`
			WrappingAdd($num),
			/// Substracts using `wrapping_sub()`
			WrappingSub($num),
			/// Divides using `wrapping_div()`
			WrappingDiv($num),
			/// Multiplies using `wrapping_mul()`
			WrappingMul($num),
			/// Wrapping (modular) remainder.
			/// regular remainder calculation. There’s no way wrapping could ever happen.
			/// This function exists, so that all operations are accounted for in the wrapping operations.
			WrappingRem($num),
			#[doc = "This copies the `Reader`'s [`" $num "`] into `self`"]
			CopyReader
		}

		impl Apply<[<Patch $num:camel>]> for $num {
			fn apply(patch: &mut [<Patch $num:camel>], writer: &mut Self, reader: &Self) {
				match patch {
					[<Patch $num:camel>]::Add(num) => *writer += *num,
					[<Patch $num:camel>]::Sub(num) => *writer -= *num,
					[<Patch $num:camel>]::Div(num) => *writer /= *num,
					[<Patch $num:camel>]::Mul(num) => *writer *= *num,
					[<Patch $num:camel>]::Mod(num) => *writer %= *num,
					[<Patch $num:camel>]::Pow(num) => *writer = writer.pow(*num),
					[<Patch $num:camel>]::SaturatingAdd(num) => *writer = writer.saturating_add(*num),
					[<Patch $num:camel>]::SaturatingSub(num) => *writer = writer.saturating_sub(*num),
					[<Patch $num:camel>]::SaturatingDiv(num) => *writer = writer.saturating_div(*num),
					[<Patch $num:camel>]::SaturatingMul(num) => *writer = writer.saturating_mul(*num),
					[<Patch $num:camel>]::SaturatingPow(num) => *writer = writer.saturating_pow(*num),
					[<Patch $num:camel>]::WrappingAdd(num) => *writer = writer.wrapping_add(*num),
					[<Patch $num:camel>]::WrappingSub(num) => *writer = writer.wrapping_sub(*num),
					[<Patch $num:camel>]::WrappingDiv(num) => *writer = writer.wrapping_div(*num),
					[<Patch $num:camel>]::WrappingMul(num) => *writer = writer.wrapping_mul(*num),
					[<Patch $num:camel>]::WrappingRem(num) => *writer = writer.wrapping_rem(*num),
					[<Patch $num:camel>]::CopyReader => *writer = *reader,
				}
			}
		}
	})*};
}

impl_num! {
	u8,
	u16,
	u32,
	u64,
	u128,
	usize,
	i8,
	i16,
	i32,
	i64,
	i128,
	isize,
}