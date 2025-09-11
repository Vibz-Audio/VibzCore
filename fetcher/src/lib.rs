struct MediaSource {}

enum MediaSourceError {
    UnsupportedFormae,
}

impl MediaSource {
    fn try_new() -> Result<Self, MediaSourceError> {
        todo!("Implement")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}
