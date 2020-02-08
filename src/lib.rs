use std::io::Error;
use std::io::Read;
use std::sync::mpsc::channel;
use std::thread;
use std::time::{Duration, Instant};

struct XXX {
    reader: dyn Read
}

pub trait ReadUntilOrTimeout {
    fn read_until_or_timeout<F>(self, predicate: F) -> Result<Vec<u8>, Error>
    where
        F: Fn(&mut [u8]) -> bool,
        F: Send;
}

impl<T: 'static> ReadUntilOrTimeout for T
where
    T: Read,
    T: Send,
{
    fn read_until_or_timeout<F>(mut self, predicate: F) -> Result<Vec<u8>, Error>
    where
        F: Fn(&mut [u8]) -> bool,
    {
        let (tx, rx) = channel();

        thread::spawn(move || {
            loop {

                thread::sleep(Duration::from_millis(100));

                let mut buf = [0_u8; 1];
                let _ = self.read(&mut buf).expect("Failed to read into buf");
                match tx.send(buf[0]) {
                    Ok(_) => {},
                    Err(_) => {}
                }
            }
        });

        let now = Instant::now();
        let mut result = vec![];
        loop {

            if now.elapsed().as_secs() > 2 {
                return Err(Error::from_raw_os_error(1));
            }

            thread::sleep(Duration::from_millis(100));

            match rx.try_recv() {
                Ok(data) => result.push(data),
                Err(_) => continue
            }

            if predicate(&mut result) {
                return Ok(result);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use std::time::Duration;
    

    #[test]
    fn it_filters_based_on_predicate() {
        let cursor = Cursor::new(vec![1, 2, 3, 4, 5, 6, 7]);
        let result = cursor.read_until_or_timeout(|x| x.contains(&5));
        assert_eq!(result.unwrap(), vec![1, 2, 3, 4, 5]);
    }

    #[test]
    fn it_times_out_when_there_are_no_matches() {
        let (tx, rx) = channel();

        thread::spawn(move || {
            let cursor = Cursor::new(vec![1, 2, 3, 4, 5, 6, 7]);
            let r = cursor.read_until_or_timeout(|x| x.contains(&10));
            tx.send(r).unwrap();
        });

        let res = rx.recv_timeout(Duration::from_secs(5)).unwrap();

        assert_eq!(res.is_err(), true);
    }
}
