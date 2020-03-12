use std::io::Read;
use std::sync::mpsc::{sync_channel, Receiver, TryRecvError};
use std::thread;
use std::time::{Duration, Instant};

const CHANNEL_BUFFER_SIZE_BYTES: usize = 1024;
const SLEEP_DURATION_WHEN_AWAITING_DATA_MS: u64 = 1;

pub struct ReadUntil {
    receiver: Receiver<u8>,
    timeout: Duration,
}

#[derive(Debug)]
pub enum ReadUntilError {
    Timeout,
    Disconnected
}

impl ReadUntil {
    pub fn new<T>(mut reader: T, timeout: Duration) -> ReadUntil
    where
        T: Read + Send + Sync + 'static,
    {
        let (tx, rx) = sync_channel(CHANNEL_BUFFER_SIZE_BYTES);

        thread::spawn(move || loop {
            let mut buffer = [0_u8; 1];
            let _ = reader.read(&mut buffer).expect("Failed to read a byte");

            match tx.send(buffer[0]) {
                Ok(_) => {}
                Err(_) => break,
            }
        });

        ReadUntil {
            receiver: rx,
            timeout,
        }
    }
}

impl ReadUntilOrTimeout for ReadUntil {
    fn until<F>(&self, predicate: F) -> Result<Vec<u8>, ReadUntilError>
    where
        F: Fn(&mut [u8]) -> bool,
    {
        let now = Instant::now();
        let mut result = vec![];
        loop {

            if now.elapsed() > self.timeout {
                return Err(ReadUntilError::Timeout);
            }

            match self.receiver.try_recv() {
                Ok(data) => result.push(data),
                Err(err) => {
                    match err {
                        TryRecvError::Empty => thread::sleep(Duration::from_millis(SLEEP_DURATION_WHEN_AWAITING_DATA_MS)),
                        TryRecvError::Disconnected => return Err(ReadUntilError::Disconnected),
                    }
                },
            }

            if predicate(&mut result) {
                return Ok(result);
            }
        }
    }
}

pub trait ReadUntilOrTimeout {
    fn until<F>(&self, predicate: F) -> Result<Vec<u8>, ReadUntilError>
    where
        F: Fn(&mut [u8]) -> bool,
        F: Send;

    fn until_contains(&self, pattern: String) -> Result<Vec<u8>, ReadUntilError> {
            self.until(|data| String::from_utf8_lossy(data).contains(&pattern))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use std::sync::mpsc::channel;

    #[test]
    fn it_filters_based_on_predicate() {
        let cursor = Cursor::new(vec![1, 2, 3, 4, 5, 6, 7]);
        let timeout = Duration::from_secs(2);
        let reader = ReadUntil::new(cursor, timeout);

        let result1 = &reader.until(|x| x.contains(&5)).unwrap();
        let result2 = &reader.until(|x| x.contains(&7)).unwrap();

        assert_eq!(result1, &vec![1, 2, 3, 4, 5]);
        assert_eq!(result2, &vec![6, 7]);
    }

    #[test]
    fn it_times_out_when_there_are_no_matches() {
        let (tx, rx) = channel();

        thread::spawn(move || {
            let cursor = Cursor::new(vec![1, 2, 3, 4, 5, 6, 7]);
            let timeout = Duration::from_millis(10);

            let reader = ReadUntil::new(cursor, timeout);
            let result = reader.until(|x| x.contains(&10));

            tx.send(result).unwrap();
        });

        let result = rx.recv_timeout(Duration::from_millis(100)).unwrap();

        assert_eq!(result.is_err(), true);
    }

    #[test]
    fn it_filters_based_on_string() {
        let cursor = Cursor::new(String::from("hello world"));
        let expected_data = &"hello wo".as_bytes().to_vec();
        let timeout = Duration::from_secs(2);
        let reader = ReadUntil::new(cursor, timeout);

        let result = &reader.until_contains("llo wo".to_string()).unwrap();

        assert_eq!(result, expected_data);
    }
}
