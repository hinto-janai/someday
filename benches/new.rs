// use criterion::{black_box, Criterion};
// use std::time::Duration;
// use std::collections::{HashMap,BTreeMap};
// use someday::patch::*;

// macro_rules! new {
// 	($func_name:ident, $func_lit:literal, $t:ty, $patch:ty) => {
// 		pub fn $func_name(c: &mut Criterion) {
// 			c
// 				.benchmark_group("new")
// 				.noise_threshold(0.05)
// 				.sample_size(1000)
// 				.measurement_time(Duration::from_secs(30))
// 				.bench_function($func_lit, |b| b.iter(||
// 			{
// 				let (r, w) = black_box(someday::new::<$t, $patch>(Default::default()));
// 			}));
// 		}
// 	};
// }

// new!(new_hashmap_usize_string, "new_hashmap_usize_string", HashMap<usize, String>, PatchHashMap<usize, String>);
// new!(new_btreemap_usize_string, "new_btreemap_usize_string", BTreeMap<usize, String>, PatchBTreeMap<usize, String>);
// new!(new_vec_string, "new_vec_string", Vec<String>, PatchVec<String>);
// new!(new_string, "new_string", String, PatchString);
// new!(new_usize, "new_usize", usize, PatchUsize);