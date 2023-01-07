use std::ffi::c_void;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::io::{AsRawFd, RawFd};
use std::path::Path;

#[derive(Debug)]
struct TransmissionData<'a> {
    buffer: crate::phy::BufferHandle<'a>,
    length: usize,
    cursor: usize,
}

#[derive(Debug)]
pub struct LinuxRs485Phy<'a> {
    fd: RawFd,
    tx: Option<TransmissionData<'a>>,
    rx: Option<TransmissionData<'a>>,
}

impl LinuxRs485Phy<'_> {
    #[inline]
    pub fn new<P: AsRef<Path>>(serial_port: P) -> Self {
        Self::new_inner(&serial_port.as_ref())
    }

    fn new_inner(serial_port: &Path) -> Self {
        // open serial port non-blocking
        let path = std::ffi::CString::new(serial_port.as_os_str().as_bytes()).unwrap();
        let fd = unsafe { libc::open(path.as_ptr() as *const i8, libc::O_RDWR | libc::O_NONBLOCK) };

        if fd < 0 {
            let error = std::io::Error::last_os_error();
            Result::<(), _>::Err(error).unwrap();
        }

        // rs485::SerialRs485::new()
        //     .set_enabled(true)
        //     .set_rts_on_send(true)
        //     .set_rts_after_send(false)
        //     .set_rx_during_tx(false)
        //     .set_on_fd(fd)
        //     .unwrap();

        Self {
            fd,
            tx: None,
            rx: None,
        }
    }

    fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
        // SAFETY: Just writing a known buffer into the file.
        match unsafe { libc::write(self.fd, buffer.as_ptr() as *const c_void, buffer.len()) } {
            -1 => {
                let err = std::io::Error::last_os_error();
                if err.kind() == std::io::ErrorKind::WouldBlock {
                    Ok(0)
                } else {
                    Err(err)
                }
            }
            written => Ok(written as usize),
        }
    }
}

impl<'a> super::ProfibusPhy<'a> for LinuxRs485Phy<'a> {
    fn schedule_tx<'b>(&'b mut self, data: super::BufferHandle<'a>, length: usize)
    where
        'a: 'b,
    {
        assert!(self.tx.is_none());
        let mut tx = TransmissionData {
            buffer: data,
            length,
            cursor: 0,
        };

        let written = self.write(&tx.buffer[..tx.length]).unwrap();
        debug_assert!(written <= tx.length);
        tx.cursor += written;
        log::debug!("TX cursor: {}", tx.cursor);

        self.tx = Some(tx);
    }

    fn poll_tx(&mut self) -> Option<super::BufferHandle<'a>> {
        match self.tx.take() {
            Some(tx) if tx.length == tx.cursor => Some(tx.buffer),
            Some(mut tx) => {
                let written = self.write(&tx.buffer[tx.cursor..tx.length]).unwrap();
                debug_assert!(written <= tx.length - tx.cursor);
                tx.cursor += written;
                log::debug!("TX cursor (poll): {}", tx.cursor);

                self.tx = Some(tx);
                None
            }
            None => panic!("polled without an ongoing tx!"),
        }
    }

    fn schedule_rx(&'a mut self, data: super::BufferHandle<'a>) {
        self.rx = Some(TransmissionData {
            buffer: data,
            length: 0, // unused
            cursor: 0,
        });
    }

    fn peek_rx(&mut self) -> &[u8] {
        if let Some(rx) = self.rx.as_ref() {
            &rx.buffer[..rx.cursor]
        } else {
            &[]
        }
    }

    fn poll_rx(&mut self) -> (super::BufferHandle, usize) {
        todo!()
    }
}
