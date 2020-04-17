use crate::{data::*, timeout_map::TimeoutMap};
use statrs::function::erf::erfc_inv;
use std::{f32::consts::*, time::*};

const MAX_LATENCY: Duration = Duration::from_millis(500);

fn inverse_q_of_probability(misses_per_sec: f32, notifs_per_sec: f32) -> f32 {
    let miss_probability = misses_per_sec / notifs_per_sec;
    // Q function: https://en.wikipedia.org/wiki/Q-function
    // miss prob = Q(target_latency / stddev)
    // Q^-1(miss prob) = sqrt(2) * erfc^-1(2 * miss prob)
    SQRT_2 * erfc_inv(2_f64 * miss_probability as f64) as f32
}

fn update_latency_average(
    old_latency_average_s: f32,
    history_count: f32,
    new_latency_sample_s: f32,
) -> f32 {
    (old_latency_average_s * history_count + new_latency_sample_s) / (history_count + 1_f32)
}

fn update_latency_variance(
    old_latency_variance_s: f32,
    old_latency_average_s: f32,
    history_count: f32,
    new_latency_sample_s: f32,
) -> f32 {
    let deviation = new_latency_sample_s - old_latency_average_s;
    (old_latency_variance_s * history_count + deviation * deviation) / (history_count + 1_f32)
}

pub struct EventTiming {
    latency_desc: LatencyDesc,
    unmatched_push_times: TimeoutMap<u64, Instant>,
    unmatched_pop_times: TimeoutMap<u64, Instant>,
    inverse_q_of_prob: f32,
    history_count: f32,
    latency_average_s: f32,
    latency_variance_s: f32,
}

impl EventTiming {
    pub fn new(latency_desc: LatencyDesc, notifs_per_sec: f32) -> Self {
        let history_mean_lifetime_s_f32 = latency_desc.history_mean_lifetime_s as f32;
        let latency_average_s = latency_desc.default_ms as f32 / 1000_f32;

        let inverse_q_of_prob = match latency_desc.mode {
            LatencyMode::Automatic {
                expected_misses_per_hour,
            } => {
                let accepted_misses_per_sec = (expected_misses_per_hour * 60 * 60) as _;
                inverse_q_of_probability(accepted_misses_per_sec, notifs_per_sec)
            }
            LatencyMode::Manual => 1_f32,
        };

        Self {
            latency_desc,
            unmatched_push_times: TimeoutMap::new(MAX_LATENCY),
            unmatched_pop_times: TimeoutMap::new(MAX_LATENCY),
            inverse_q_of_prob,
            history_count: history_mean_lifetime_s_f32 * notifs_per_sec,
            latency_average_s,
            latency_variance_s: latency_average_s / inverse_q_of_prob,
        }
    }

    pub fn reset_notifs_per_sec(&mut self, notifs_per_sec: f32) {
        if let LatencyMode::Automatic {
            expected_misses_per_hour,
            ..
        } = self.latency_desc.mode
        {
            let accepted_misses_per_sec = expected_misses_per_hour * 60 * 60;
            self.inverse_q_of_prob =
                inverse_q_of_probability(accepted_misses_per_sec as _, notifs_per_sec);
        }
        self.history_count = self.latency_desc.history_mean_lifetime_s as f32 * notifs_per_sec;
    }

    // This method call can be skipped for some id or can be in any order.
    pub fn notify_push(&mut self, id: u64) {
        self.unmatched_push_times.insert(id, Instant::now());
    }

    // This should be called for every id in increasing order.
    // Returns a latency correction offset to be used to delay or anticipate the events that lead
    // to the `notify_push` calls.
    pub fn notify_pop(&mut self, id: u64) {
        let now = Instant::now();
        self.unmatched_pop_times.insert(id, now);

        let mut pop_ids_to_be_removed = vec![];
        for &id in self.unmatched_pop_times.keys() {
            let maybe_time = self.unmatched_push_times.remove(&id).map(|(_, t)| t);

            if let Some(time) = maybe_time {
                let latency_sample_s = (now - time).as_secs_f32();
                self.latency_average_s = update_latency_average(
                    self.latency_average_s,
                    self.history_count,
                    latency_sample_s,
                );
                if matches!(self.latency_desc.mode, LatencyMode::Automatic { .. }) {
                    self.latency_variance_s = update_latency_variance(
                        self.latency_variance_s,
                        self.latency_average_s,
                        self.history_count,
                        latency_sample_s,
                    );
                }
                pop_ids_to_be_removed.push(id);
            }
        }

        self.unmatched_push_times.remove_expired();
        self.unmatched_pop_times.remove_expired();
        for id in pop_ids_to_be_removed {
            self.unmatched_pop_times.remove(&id);
        }
    }

    // equivalent to notify_push() + notify_pop() calls with `latency` interval between them.
    pub fn notify_latency(&mut self, latency: Duration) {
        let latency_sample_s = latency.as_secs_f32();

        self.latency_average_s =
            update_latency_average(self.latency_average_s, self.history_count, latency_sample_s);

        if matches!(self.latency_desc.mode, LatencyMode::Automatic { .. }) {
            self.latency_variance_s = update_latency_variance(
                self.latency_variance_s,
                self.latency_average_s,
                self.history_count,
                latency_sample_s,
            );
        }
    }

    pub fn target_latency(&self) -> Duration {
        // miss prob = Q(target latency / stddev)
        // target latency = sqrt(latency variance) * Q^-1(miss prob)
        Duration::from_secs_f32(self.latency_variance_s.sqrt() * self.inverse_q_of_prob)
        // Note: if manual latency is choosen, latency_variance_s is default latency and
        // inverse_q_of_prob is 1.
    }

    pub fn average_latency(&self) -> Duration {
        Duration::from_secs_f32(self.latency_average_s)
    }
}
