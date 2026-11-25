//! What writes batches to the injector's pipe: a thread of its own, fed from a bounded
//! queue (docs/multi-pc-service-windows.md).
//!
//! A pipe write blocks while the injector does not read, and the service's deciding
//! loop — which also serves the SCM's stop, the session changes and the withdrawals —
//! must never wait on it. So the loop only queues, and never waits: a queue full is an
//! injector that stopped reading, and its session is ended instead (the injector
//! stopped, which closes its input and makes it release everything), rather than
//! waited on.

use std::io::Write;
use std::sync::mpsc;

/// Batches waiting for the injector: a few dozen milliseconds of a fast mouse.
pub const QUEUE: usize = 64;

pub struct Writer {
    frames: mpsc::SyncSender<Vec<u8>>,
}

impl Writer {
    /// Starts writing to `pipe` on a thread of its own, which ends with the writer, or
    /// at the first write that fails.
    pub fn start(mut pipe: impl Write + Send + 'static) -> Writer {
        let (frames, queued) = mpsc::sync_channel::<Vec<u8>>(QUEUE);
        std::thread::spawn(move || {
            for frame in queued {
                if pipe.write_all(&frame).and_then(|()| pipe.flush()).is_err() {
                    break;
                }
            }
        });
        Writer { frames }
    }

    /// Queues one batch, framed; `false` when the queue is full or the writer gone:
    /// the session is to end.
    pub fn send(&self, batch: &[u8]) -> bool {
        let mut frame = (batch.len() as u32).to_le_bytes().to_vec();
        frame.extend_from_slice(batch);
        self.frames.try_send(frame).is_ok()
    }

    /// Queues the end of a session (`Leave`). `false` when it could not be: then the
    /// injector must be stopped, or what the session holds stays pressed.
    pub fn end(&self) -> bool {
        self.send(&lbm_ipc::input::encode(&[
            lbm_ipc::input::InputEvent::Leave,
        ]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    /// A pipe that takes nothing until `open` is set, then records what it is given.
    #[derive(Clone, Default)]
    struct Pipe {
        open: Arc<(Mutex<bool>, std::sync::Condvar)>,
        written: Arc<Mutex<Vec<u8>>>,
        /// A write is under way (and, while closed, blocked).
        writing: Arc<std::sync::atomic::AtomicBool>,
    }

    impl Write for Pipe {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.writing
                .store(true, std::sync::atomic::Ordering::SeqCst);
            let (open, opened) = &*self.open;
            let mut is_open = open.lock().unwrap();
            while !*is_open {
                is_open = opened.wait(is_open).unwrap();
            }
            self.written.lock().unwrap().extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    impl Pipe {
        fn open(&self) {
            let (open, opened) = &*self.open;
            *open.lock().unwrap() = true;
            opened.notify_all();
        }
    }

    #[test]
    fn a_full_queue_refuses_the_leave_so_the_session_is_ended_otherwise() {
        let pipe = Pipe::default();
        let writer = Writer::start(pipe.clone());
        // One frame is taken by the writing thread, which then blocks on the pipe: from
        // then on nothing leaves the queue.
        assert!(writer.send(&[1]));
        while !pipe.writing.load(std::sync::atomic::Ordering::SeqCst) {
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        let mut queued = 0;
        while writer.send(&[1]) {
            queued += 1;
            assert!(queued <= QUEUE + 2, "the queue does not fill");
        }
        assert!(!writer.end(), "a Leave found room in a full queue");
        pipe.open();
    }

    #[test]
    fn a_leave_with_room_is_written_after_the_batches() {
        let pipe = Pipe::default();
        pipe.open();
        let writer = Writer::start(pipe.clone());
        assert!(writer.send(&[9]));
        assert!(writer.end());
        drop(writer);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        let leave = lbm_ipc::input::encode(&[lbm_ipc::input::InputEvent::Leave]);
        let mut expected = vec![1, 0, 0, 0, 9];
        expected.extend_from_slice(&(leave.len() as u32).to_le_bytes());
        expected.extend_from_slice(&leave);
        while *pipe.written.lock().unwrap() != expected {
            assert!(std::time::Instant::now() < deadline, "not written in time");
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }

    #[test]
    fn a_pipe_that_fails_ends_the_writer_and_sends_are_refused() {
        struct Broken;
        impl Write for Broken {
            fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
                Err(std::io::ErrorKind::BrokenPipe.into())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let writer = Writer::start(Broken);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while writer.send(&[1]) {
            assert!(std::time::Instant::now() < deadline, "sends still taken");
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }
}
