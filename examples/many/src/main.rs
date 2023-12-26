// This is a comparison of:
//
// `RwLock<Many>`
//     vs.
// someday's `Writer<Many>` & `Reader<Many>`
//
// The test:
// 1. Create a writer/reader `Many` (defined below)
// 2. Readers will continually acquire a lock as fast as possible
// 3. The Writer will continue until it has pushed `N` elements
//
// The test below is quite unrealistic (the readers are
// continually acquiring a lock as fast as possible) however
// the point is that someday achieves better parallelism
// by being lock-free.

use someday::*;
use std::sync::*;
use std::time::*;
use std::hint::black_box;
use std::collections::HashMap;

// How many parallel reader threads to spawn.
const READER_THREADS: usize = 32;
// What number the writer must reach to "finish".
const N: usize = 5_000;

#[derive(Clone)]
struct Many {
    vec: Vec<String>,
    hashmap: HashMap<usize, usize>,
    string: String,
}

impl Many {
    fn new() -> Self {
        Self {
            vec: Vec::with_capacity(N),
            hashmap: HashMap::with_capacity(N),
            string: String::with_capacity(N),
        }
    }
}

fn main() {
    // Time `RwLock<Vec>`
    let rwlock = black_box(rwlock());
    // Time `someday`'s Vec.
    let someday = black_box(someday());

    println!("rwlock: {rwlock}");
    println!("someday: {someday}");

    // `RwLock` is anywhere from 5x-100x slower in these
    // (unrealistic) highly contentious lock situations.
    assert!(rwlock > (someday * 5.0));
}

#[cold]
#[inline(never)]
fn rwlock() -> f64 {
	let many = Arc::new(RwLock::new(Many::new()));
    let barrier = Arc::new(Barrier::new(READER_THREADS + 1));

    // Spawn a bunch of readers.
	for _ in 0..READER_THREADS {
		let reader_many: Arc<RwLock<Many>> = Arc::clone(&many);
        let barrier = Arc::clone(&barrier);
        // Each reader will continually acquire
        // a reader lock as fast as possible,
        // putting heavy contention on the writer.
		std::thread::spawn(move || {
            barrier.wait();
			loop { let reader_lock = black_box(reader_many.read().unwrap()); }
		});
	}

    // Wait until all Readers are ready.
    barrier.wait();

    let now = Instant::now();

    // The Writer will acquire a writer lock
    // and insert up until 10,000.
    //
    // The Readers are putting heavy contention
    // on the lock so the Writer will have to
    // wait quite a while before getting a turn.
	for i in 0..N {
		let mut many = many.write().unwrap();
        let string = format!("{i}");
        many.string.push_str(&string);
        many.hashmap.insert(i, i);
        many.vec.push(string);
	}

    now.elapsed().as_secs_f64()
}

#[cold]
#[inline(never)]
fn someday() -> f64 {
    // Create a Vec backed by `someday`.
	let (reader, mut writer) = someday::new(Many::new());
    let barrier = Arc::new(Barrier::new(READER_THREADS + 1));

    // Spawn a bunch of readers.
	for _ in 0..READER_THREADS {
		let reader_many = reader.clone();
        let barrier = Arc::clone(&barrier);
        // Each reader will continually
        // acquire read access to the Vec
        // as fast as possible.
		std::thread::spawn(move || {
            barrier.wait();
			loop { let reader_lock = black_box(reader_many.head()); }
		});
	}

    // Wait until all Readers are ready.
    barrier.wait();

    let now = Instant::now();

    // The Writer will write up until 10,000,
    // lock-free, cloning and re-acquiring data
    // if needed.
	for i in 0..N {
		writer.add(|w, _| {
            let i = w.vec.len() + 1;
            let string = format!("{i}");
            w.string.push_str(&string);
            w.hashmap.insert(i, i);
            w.vec.push(string);
        });
		writer.commit();
		writer.push();
	}

    now.elapsed().as_secs_f64()
}