use std::time::Duration;

#[derive(Debug)]
enum DecodeError {}

#[derive(Debug)]
pub struct DecoderError(DecodeError);

pub trait Decoder {
    fn decode(&mut self) -> Result<Vec<f32>, DecoderError>;
    fn seek(&mut self, to: Duration) -> Result<(), DecoderError>;
}
