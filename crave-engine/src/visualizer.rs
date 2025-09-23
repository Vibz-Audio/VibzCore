use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

pub struct AudioVisualizer {
    sample_history: Arc<Mutex<VecDeque<f32>>>,
    history_size: usize,
    bar_count: usize,
}

impl AudioVisualizer {
    pub fn new(history_size: usize, bar_count: usize) -> Self {
        Self {
            sample_history: Arc::new(Mutex::new(VecDeque::with_capacity(history_size))),
            history_size,
            bar_count,
        }
    }

    pub fn add_sample(&self, sample: f32) {
        let mut history = self.sample_history.lock().unwrap();

        if history.len() >= self.history_size {
            history.pop_front();
        }

        history.push_back(sample);
    }

    pub fn render_bars(&self) -> String {
        let history = self.sample_history.lock().unwrap();

        if history.is_empty() {
            return "│".repeat(self.bar_count);
        }

        let chunk_size = history.len().max(1) / self.bar_count.max(1);
        let mut bars = Vec::new();

        for i in 0..self.bar_count {
            let start = i * chunk_size;
            let end = ((i + 1) * chunk_size).min(history.len());

            if start >= history.len() {
                bars.push(0.0);
                continue;
            }

            let chunk: Vec<f32> = history.range(start..end).cloned().collect();
            let rms = if chunk.is_empty() {
                0.0
            } else {
                let sum_squares: f32 = chunk.iter().map(|x| x * x).sum();
                (sum_squares / chunk.len() as f32).sqrt()
            };

            bars.push(rms);
        }

        let max_amplitude = bars.iter().cloned().fold(0.0f32, f32::max).max(0.001);

        bars.iter()
            .map(|&amplitude| {
                let normalized = (amplitude / max_amplitude).min(1.0);
                let height = (normalized * 8.0) as usize;

                match height {
                    0 => "│",
                    1 => "▁",
                    2 => "▂", 
                    3 => "▃",
                    4 => "▄",
                    5 => "▅",
                    6 => "▆",
                    7 => "▇",
                    _ => "█",
                }
            })
            .collect::<String>()
    }

    pub fn render_waveform(&self) -> String {
        let history = self.sample_history.lock().unwrap();

        if history.is_empty() {
            return "─".repeat(80);
        }

        let width = 80;
        let height = 20;
        let mut grid = vec![vec![' '; width]; height];

        let chunk_size = history.len().max(1) / width;

        for x in 0..width {
            let start = x * chunk_size;
            let end = ((x + 1) * chunk_size).min(history.len());

            if start >= history.len() {
                continue;
            }

            let chunk: Vec<f32> = history.range(start..end).cloned().collect();
            if chunk.is_empty() {
                continue;
            }

            let avg = chunk.iter().sum::<f32>() / chunk.len() as f32;
            let y = ((avg + 1.0) / 2.0 * height as f32) as usize;
            let y = y.min(height - 1);

            grid[height - 1 - y][x] = '█';
        }

        grid.iter()
            .map(|row| row.iter().collect::<String>())
            .collect::<Vec<String>>()
            .join("\n")
    }

    pub fn get_sample_history(&self) -> Arc<Mutex<VecDeque<f32>>> {
        self.sample_history.clone()
    }
}

pub fn display_visualization(visualizer: &AudioVisualizer) {
    print!("\x1B[2J\x1B[H"); // Clear screen and move cursor to top

    let bars = visualizer.render_bars();
    println!("Audio Visualization:");
    println!("┌{}┐", "─".repeat(bars.len()));
    println!("│{}│", bars);
    println!("└{}┘", "─".repeat(bars.len()));

    let peak_level = {
        let sample_history = visualizer.get_sample_history();
        let history = sample_history.lock().unwrap();
        if history.is_empty() {
            0.0
        } else {
            history.iter().map(|x| x.abs()).fold(0.0f32, f32::max)
        }
    };

    println!("Peak Level: {:.3}", peak_level);
    println!("Press 'p' to toggle play/pause, 'q' to quit");
}

