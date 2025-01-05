use std::io::{self, Write};

/// `MultiWriter` одновременно пишет во все свои `writers`.
pub struct MultiWriter<W1, W2> {
    w1: W1,
    w2: W2,
}

impl<W1: Write, W2: Write> MultiWriter<W1, W2> {
    pub fn new(w1: W1, w2: W2) -> Self {
        Self { w1, w2 }
    }
}

impl<W1: Write, W2: Write> Write for MultiWriter<W1, W2> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.w1.write(buf)?;
        self.w2.write(buf)?;

        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.w1.flush()?;
        self.w2.flush()
    }
}
