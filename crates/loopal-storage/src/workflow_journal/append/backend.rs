use std::io::{self, Write};

pub(crate) trait AppendOutput: Write {
    fn byte_len(&self) -> io::Result<u64>;
    fn sync_data(&self) -> io::Result<()>;
}

impl AppendOutput for std::fs::File {
    fn byte_len(&self) -> io::Result<u64> {
        self.metadata().map(|metadata| metadata.len())
    }

    fn sync_data(&self) -> io::Result<()> {
        std::fs::File::sync_data(self)
    }
}
