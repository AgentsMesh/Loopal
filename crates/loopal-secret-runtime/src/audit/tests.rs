use std::io::{self, Write};

use tempfile::tempdir;

use super::*;

struct FailingWriter {
    write: bool,
    flush: bool,
    sync: bool,
}

impl Write for FailingWriter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        if self.write {
            Err(io::Error::other("write"))
        } else {
            Ok(buffer.len())
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        if self.flush {
            Err(io::Error::other("flush"))
        } else {
            Ok(())
        }
    }
}

impl AuditWriter for FailingWriter {
    fn sync_data(&self) -> io::Result<()> {
        if self.sync {
            Err(io::Error::other("sync"))
        } else {
            Ok(())
        }
    }
}

fn append_with(writer: FailingWriter) -> AuditError {
    let dir = tempdir().unwrap();
    JsonlAuditSink::new(dir.path().to_path_buf())
        .append_with(
            SerializedOp::Runtime(RuntimeOp::Resolved),
            "post_effect",
            "name",
            &AuditMetadata::default(),
            |_| Ok(writer),
        )
        .unwrap_err()
}

#[test]
fn append_maps_write_flush_and_sync_failures() {
    assert!(matches!(
        append_with(FailingWriter {
            write: true,
            flush: false,
            sync: false,
        }),
        AuditError::Write { .. }
    ));
    assert!(matches!(
        append_with(FailingWriter {
            write: false,
            flush: true,
            sync: false,
        }),
        AuditError::Flush { .. }
    ));
    assert!(matches!(
        append_with(FailingWriter {
            write: false,
            flush: false,
            sync: true,
        }),
        AuditError::Sync { .. }
    ));
}
