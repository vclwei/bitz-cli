use std::{sync::Arc, time::Instant};

use drillx::equix;
use log::info;
use solana_rpc_client::spinner;

use crate::{args::BenchmarkArgs, Miner};

const TEST_DURATION: i64 = 30;

impl Miner {
    pub async fn benchmark(&self, args: BenchmarkArgs) {
        // Check num threads
        let cores = self.parse_cores(args.cores);
        self.check_num_cores(cores);

        // Dispatch job to each thread
        let challenge = [0; 32];
        let progress_bar = Arc::new(spinner::new_progress_bar());
        progress_bar.set_message(format!(
            "Benchmarking. This will take {} sec...",
            TEST_DURATION
        ));
        let core_count = core_affinity::get_core_ids()
            .map(|ids| ids.len())
            .unwrap_or(1);
        info!("core_count: {}", core_count);
        
        let handles: Vec<_> = (0..cores)
            .map(|i| {
                std::thread::spawn({
                    move || {
                        let timer = Instant::now();
                        let first_nonce = u64::MAX
                            .saturating_div(cores)
                            .saturating_mul(i);
                        let mut nonce = first_nonce;
                        let mut memory = equix::SolverMemory::new();
                        let mut hash_count = 0u64;

                        // Pin to core if possible
                        if let Some(core_ids) = core_affinity::get_core_ids() {
                            let physical_core_index = (i as usize) / core_count;
                            if let Some(core_id) = core_ids.get(physical_core_index) {
                                let _ = core_affinity::set_for_current(*core_id);
                            }
                        }

                        // 根据线程在核心内的索引调整起始 nonce
                        let thread_in_core = (i as usize) % core_count;
                        if thread_in_core == 1 {
                            // 第二个线程从中间开始
                            nonce = nonce.saturating_add(u64::MAX / (cores * 2));
                        }

                        loop {
                            // Create hash
                            let _hx = drillx::hash_with_memory(
                                &mut memory,
                                &challenge,
                                &nonce.to_le_bytes(),
                            );
                            hash_count += 1;

                            // Increment nonce
                            nonce += 1;

                            // Exit if time has elapsed
                            if (timer.elapsed().as_secs() as i64).ge(&TEST_DURATION) {
                                break;
                            }
                        }

                        // Return hash count
                        hash_count
                    }
                })
            })
            .collect();

        // Join handles and return best nonce
        let mut total_hashes = 0u64;
        for h in handles {
            if let Ok(count) = h.join() {
                total_hashes += count;
            }
        }

        // Update log
        let hashes_per_second = total_hashes / TEST_DURATION as u64;
        progress_bar.finish_with_message(format!(
            "Hashpower: {} H/sec",
            hashes_per_second,
        ));
    }
}
