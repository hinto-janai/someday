use criterion::{criterion_group, criterion_main};

mod new;

criterion_group! {
	benches,
	crate::new::new_hashmap_usize_string,
	crate::new::new_btreemap_usize_string,
	crate::new::new_vec_string,
	crate::new::new_vec_string,
	crate::new::new_string,
	crate::new::new_usize,
}

criterion_main!(benches);
