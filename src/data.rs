//! Data generation for all visualization types

/// Time series data (existing) - 200 validators with oscillation
pub struct TimeSeriesData {
    pub series: Vec<Vec<f32>>,
    pub time_index: usize,
    pub max_points: usize,
    base_values: Vec<f32>,
}

impl TimeSeriesData {
    pub fn new(num_series: usize, max_points: usize) -> Self {
        let base_values: Vec<f32> = (0..num_series).map(|_| 50.0).collect();
        Self {
            series: vec![Vec::with_capacity(max_points); num_series],
            time_index: 0,
            max_points,
            base_values,
        }
    }

    pub fn tick(&mut self) {
        self.time_index += 1;

        let freq = 0.05;
        let amplitude = 7.5;  // Reduced by 25% from 10.0
        let period = 2.0 * std::f32::consts::PI / freq;

        for (i, series) in self.series.iter_mut().enumerate() {
            let last = series.last().copied().unwrap_or(self.base_values[i]);

            // First 400 validators: constant phase, strong noise
            // Last 600 validators: oscillating phase (20% of period) with noise
            let (signal, noise) = if i < 400 {
                // Mainstream: no phase shift, strong amplitude noise
                let amp_noise = (rand_f32() - 0.5) * 8.0;  // 2x bigger
                let sig = (self.time_index as f32 * freq).sin() * amplitude;
                (sig + amp_noise, (rand_f32() - 0.5) * 1.0)
            } else {
                // Phase-shifted validators 400-1023
                let idx = i - 400;  // 0-623

                // Phase oscillation: max 1% of period, oscillates -range to +range
                let phase_range = period * 0.005;  // ±0.5% = 1% total swing
                let phase_offset = idx as f32 * 0.05;  // Spread validators
                let phase_noise = (rand_f32() - 0.5) * 0.3;  // Noise so not aligned
                let phase_shift = (self.time_index as f32 * 0.01 + phase_offset).sin() * phase_range + phase_noise;

                let sig = (self.time_index as f32 * freq + phase_shift).sin() * amplitude;
                (sig, (rand_f32() - 0.5) * 0.5)
            };

            let target = self.base_values[i] + signal;
            let reversion = (target - last) * 0.1;
            let value = last + noise + reversion;

            if series.len() >= self.max_points {
                series.remove(0);
            }
            series.push(value);
        }
    }

    pub fn point_count(&self) -> usize {
        self.series.first().map_or(0, |s| s.len())
    }
}

/// Best block per validator - scatter plot
pub struct BestBlockData {
    pub blocks: Vec<u32>,           // Current best block for each validator
    pub last_update: Vec<f64>,      // When each validator last updated
    update_delays: Vec<f64>,        // Random delay for each validator
}

impl BestBlockData {
    pub fn new(num_validators: usize) -> Self {
        Self {
            blocks: vec![0; num_validators],
            last_update: vec![0.0; num_validators],
            update_delays: (0..num_validators)
                .map(|_| 0.5 + rand_f32() as f64 * 1.0)  // 0.5-1.5s delay
                .collect(),
        }
    }

    pub fn tick(&mut self, time: f64) {
        for i in 0..self.blocks.len() {
            // Check if enough time passed for this validator
            if time - self.last_update[i] >= self.update_delays[i] {
                self.blocks[i] += 1;
                self.last_update[i] = time;
                // Randomize next delay
                self.update_delays[i] = 0.5 + rand_f32() as f64 * 1.0;
            }
        }
    }
}

/// Event histogram data - bars for A and B events per validator
pub struct EventHistogramData {
    pub events_a: Vec<u32>,
    pub events_b: Vec<u32>,
}

impl EventHistogramData {
    pub fn new(num_validators: usize) -> Self {
        Self {
            events_a: vec![0; num_validators],
            events_b: vec![0; num_validators],
        }
    }

    pub fn tick(&mut self) {
        // Randomly increment events for some validators
        for i in 0..self.events_a.len() {
            // ~10% chance per validator per tick
            if rand_f32() < 0.1 {
                self.events_a[i] += 1;
            }
            if rand_f32() < 0.08 {
                self.events_b[i] += 1;
            }
        }
    }
}

fn rand_f32() -> f32 {
    js_sys::Math::random() as f32
}
