use super::placement;
use lazy_static::*;

use std::thread;

use super::load_balancer;
use crate::worker;
use crossbeam_utils::sync::ShardedLock;
use rustc_hash::FxHashMap;
use std::time::Duration;

pub struct LoadBalancer();

impl LoadBalancer {
    pub fn sample() {
        thread::Builder::new()
            .name("load-balancer-thread".to_string())
            .spawn(move || {
                loop {
                    if let Ok(mut stats) = load_balancer::stats().try_write() {
                        // Write latest downscaled mean to statistics
                        stats.mean_level = stats
                            .smp_queues
                            .values()
                            .sum::<usize>()
                            .wrapping_div(placement::get_core_ids().unwrap().len());
                    }

                    // Try sleeping for a while to wait
                    thread::sleep(Duration::new(0, 10));
                    // Yield immediately back to os so we can advance in workers
                    thread::yield_now();
                }
            })
            .expect("load-balancer couldn't start");
    }
}

#[derive(Clone)]
pub struct Stats {
    pub(crate) global_run_queue: usize,
    pub(crate) mean_level: usize,
    pub(crate) smp_queues: FxHashMap<usize, usize>,
}

unsafe impl Send for Stats {}
unsafe impl Sync for Stats {}

#[inline]
pub fn stats() -> &'static ShardedLock<Stats> {
    lazy_static! {
        static ref LB_STATS: ShardedLock<Stats> = {
            let stats = Stats {
                global_run_queue: 0,
                mean_level: 0,
                smp_queues: FxHashMap::with_capacity_and_hasher(
                    placement::get_core_ids().unwrap().len(),
                    Default::default()
                )
            };

            // Start sampler
            LoadBalancer::sample();

            // Return stats
            ShardedLock::new(stats)
        };
    }
    &*LB_STATS
}