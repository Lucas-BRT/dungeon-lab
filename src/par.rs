//! The one place that decides whether work runs in parallel. With the `parallel` feature (rayon) the helpers split a slice over the
//! thread pool; without it they are plain loops, so the single-threaded WebAssembly build has no threads, workers or special headers.
//!
//! Both give the same answer in the same order (rayon's indexed collect keeps the order), so a stage never depends on the thread count.

#[cfg(feature = "parallel")]
pub fn pmap<T: Sync, R: Send>(xs: &[T], f: impl Fn(&T) -> R + Sync + Send) -> Vec<R> {
    use rayon::prelude::*;
    if xs.len() < 2 {
        return xs.iter().map(f).collect();
    }
    xs.par_iter().map(f).collect()
}

#[cfg(not(feature = "parallel"))]
pub fn pmap<T: Sync, R: Send>(xs: &[T], f: impl Fn(&T) -> R + Sync + Send) -> Vec<R> {
    xs.iter().map(f).collect()
}

/// `pmap` for cheap items: a split is only worth it from `min` of them.
pub fn pmap_min<T: Sync, R: Send>(xs: &[T], min: usize, f: impl Fn(&T) -> R + Sync + Send) -> Vec<R> {
    if xs.len() < min {
        return xs.iter().map(f).collect();
    }
    pmap(xs, f)
}

/// `pmap_min` that flattens what each item returns, in item order.
pub fn pflat_min<T: Sync, R: Send>(xs: &[T], min: usize, f: impl Fn(&T) -> Vec<R> + Sync + Send) -> Vec<R> {
    pmap_min(xs, min, f).into_iter().flatten().collect()
}

/// Threads the pool runs (1 when the build has none).
pub fn threads() -> usize {
    #[cfg(feature = "parallel")]
    {
        rayon::current_num_threads()
    }
    #[cfg(not(feature = "parallel"))]
    {
        1
    }
}
