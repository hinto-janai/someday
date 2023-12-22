// This is a comparison of
//
// `RwLock<Vec>`
//     vs.
// someday's `Writer<Vec>` & `Reader<Vec>`
//
// The test:
// 1. Create a writer/reader `Vec<usize>`
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

// How many parallel reader threads to spawn.
const READER_THREADS: usize = 32;
// What number the writer must reach to "finish".
const N: usize = 30_000;

fn main() {
    // Time `RwLock<Vec>`
    let rwlock = black_box(rwlock());
    // Time `someday`'s Vec.
    let someday = black_box(someday());

    println!("rwlock: {rwlock}");
    println!("someday: {someday}");

    // `RwLock` is anywhere from 3x-20x slower in these
    // (unrealistic) highly contentious lock situations.
    assert!(rwlock > (someday * 3.0));
}

#[cold]
#[inline(never)]
fn rwlock() -> f64 {
	let vec = Arc::new(RwLock::new(Vec::with_capacity(N)));
    let barrier = Arc::new(Barrier::new(READER_THREADS + 1));

    // Spawn a bunch of readers.
	for _ in 0..READER_THREADS {
		let reader_vec: Arc<RwLock<Vec<usize>>> = Arc::clone(&vec);
        let barrier = Arc::clone(&barrier);
        // Each reader will continually acquire
        // a reader lock as fast as possible,
        // putting heavy contention on the writer.
		std::thread::spawn(move || {
            barrier.wait();
			loop { let reader_lock = black_box(reader_vec.read().unwrap()); }
		});
	}

    // Wait until all Readers are ready.
    barrier.wait();

    let now = Instant::now();

    // The Writer will acquire a writer lock
    // and insert up until N.
    //
    // The Readers are putting heavy contention
    // on the lock so the Writer will have to
    // wait quite a while before getting a turn.
	for i in 0..N {
		let mut vec = vec.write().unwrap();
		vec.insert(i, i);
	}

    now.elapsed().as_secs_f64()
}

#[cold]
#[inline(never)]
fn someday() -> f64 {
    // Create a Vec backed by `someday`.
	let (reader, mut writer) = someday::new(Vec::<usize>::with_capacity(N));
    let barrier = Arc::new(Barrier::new(READER_THREADS + 1));

    // Spawn a bunch of readers.
	for _ in 0..READER_THREADS {
		let reader_vec = reader.clone();
        let barrier = Arc::clone(&barrier);
        // Each reader will continually
        // acquire read access to the Vec
        // as fast as possible.
		std::thread::spawn(move || {
            barrier.wait();
			loop { let reader_lock = black_box(reader_vec.head()); }
		});
	}

    // Wait until all Readers are ready.
    barrier.wait();

    let now = Instant::now();

    // The Writer will write up until N,
    // lock-free, cloning and re-acquiring data
    // if needed.
	for i in 0..N {
		writer.add(move |vec: &mut Vec<usize>, _| {
            vec.push(i);
        });
		writer.commit();
		writer.push();
	}

    now.elapsed().as_secs_f64()
}