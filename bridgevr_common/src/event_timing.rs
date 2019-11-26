use statrs::function::erf::erfc_inv;
use std::collections::HashMap;
use std::f32::consts::*;
use std::time::*;

const MAX_LATENCY: Duration = Duration::from_millis(500);

fn inverse_q_of_probability(misses_per_sec: f32, fps: f32) -> f32 {
    let miss_probability = misses_per_sec * fps;
    // Q function: https://en.wikipedia.org/wiki/Q-function
    // miss prob = Q(target_latency / stddev)
    // Q^-1(miss prob) = sqrt(2) * erfc^-1(2 * miss prob)
    SQRT_2 * erfc_inv(2f64 * miss_probability as f64) as f32
}

fn update_latency_average(
    old_latency_average_s: f32,
    history_count: f32,
    new_latency_sample_s: f32,
) -> f32 {
    (old_latency_average_s * history_count + new_latency_sample_s)
        / (history_count + 1f32)
}

fn update_latency_variance(
    old_latency_variance_s: f32,
    old_latency_average_s: f32,
    history_count: f32,
    new_latency_sample_s: f32,
) -> f32 {
    let deviation = new_latency_sample_s - old_latency_average_s;
    (old_latency_variance_s * history_count + deviation * deviation)
        / (history_count + 1f32)
}

pub struct EventTiming {
    unmatched_submit_times: HashMap<usize, Instant>,
    unmatched_present_times: HashMap<usize, Instant>,
    inverse_q_of_prob: f32,
    history_count: f32,
    latency_average_s: f32,
    latency_variance_s: f32,
}

impl EventTiming {
    pub fn new(
        accepted_misses: u32,
        over_duration: Duration,
        fps: f32,
        defaut_latency: Duration,
        history_mean_lifetime: Duration,
    ) -> Self {
        let inverse_q_of_prob =
            inverse_q_of_probability(accepted_misses as f32 / over_duration.as_secs_f32(), fps);

        Self {
            unmatched_submit_times: HashMap::new(),
            unmatched_present_times: HashMap::new(),
            inverse_q_of_prob,
            history_count: history_mean_lifetime.as_secs_f32() * fps,
            latency_average_s: defaut_latency.as_secs_f32(),
            latency_variance_s: defaut_latency.as_secs_f32() / inverse_q_of_prob,
        }
    }

    // This method call can be skipped for some id or can be in any order.
    pub fn notify_submit(&mut self, id: usize) {
        self.unmatched_submit_times.insert(id, Instant::now());
    }

    fn get_latency_offset(&self) -> Duration {
        // miss prob = Q(target latency / stddev)
        // target latency = sqrt(latency variance) * Q^-1(miss prob)
        let target_latency_s = self.latency_variance_s.sqrt() * self.inverse_q_of_prob;

        Duration::from_secs_f32(self.latency_average_s - target_latency_s)
    }

    // This should be called for every id in increasing order.
    // Returns a latency correction offset to be used to delay or anticipate the events that lead
    // to the `notify_submit` calls.
    pub fn notify_present(&mut self, id: usize) -> Duration {
        let now = Instant::now();
        self.unmatched_present_times.insert(id, now);

        let mut present_ids_to_be_removed = vec![];
        for (&id, &present_time) in self.unmatched_present_times.iter() {
            let maybe_time = if now - present_time > MAX_LATENCY {
                Some(present_time)
            } else {
                self.unmatched_submit_times.remove(&id) // if id not found returns None
            };

            if let Some(time) = maybe_time {
                let latency_sample_s = (now - time).as_secs_f32();
                self.latency_average_s = update_latency_average(
                    self.latency_average_s,
                    self.history_count,
                    latency_sample_s,
                );
                self.latency_variance_s = update_latency_variance(
                    self.latency_variance_s,
                    self.latency_average_s,
                    self.history_count,
                    latency_sample_s,
                );
                present_ids_to_be_removed.push(id);
            }
        }
        self.unmatched_present_times
            .retain(|id, _| !present_ids_to_be_removed.contains(id));
        self.unmatched_submit_times
            .retain(|_, time| now - *time > MAX_LATENCY);

        self.get_latency_offset()
    }
}
