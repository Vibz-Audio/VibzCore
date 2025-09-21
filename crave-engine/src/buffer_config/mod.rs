use ringbuf::SharedRb;
use ringbuf::storage::Heap;

pub struct AudioBufferConfig {
    pub rb: SharedRb<Heap<f32>>,
    /** Playback sample rate */
    pub sample_rate: u32,
    /** Lookahead time in seconds */
    pub lookahead: usize,
    /** Capacity of the ring buffer */
    pub capacity: usize,
    /** Threshold to consider buffer "full" */
    pub threshold: usize,
    /** Tolerance level to trigger data requests */
    pub tolerance: usize,
}

impl AudioBufferConfig {
    pub fn new(sample_rate: u32, lookahead: usize) -> Self {
        let capacity: usize = ((sample_rate) as usize * 2) * lookahead;
        let threshold: usize = capacity - 1024; // Threshold to consider buffer "full"
        let tolerance: usize = capacity / 2;
        AudioBufferConfig {
            rb: SharedRb::<Heap<f32>>::new(capacity),
            lookahead,
            sample_rate,
            capacity,
            threshold,
            tolerance,
        }
    }
}

impl Default for AudioBufferConfig {
    fn default() -> Self {
        let sample_rate: u32 = 44100;
        let lookahead: usize = 30; // Lookahead in seconds
        let capacity: usize = ((sample_rate) as usize * 2) * lookahead;
        let threshold: usize = capacity - 1024; // Threshold to consider buffer "full"
        let tolerance: usize = capacity / 2;
        AudioBufferConfig {
            rb: SharedRb::<Heap<f32>>::new(capacity),
            lookahead,
            sample_rate,
            capacity,
            threshold,
            tolerance,
        }
    }
}
