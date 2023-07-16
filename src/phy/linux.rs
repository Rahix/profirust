use std::ffi::c_void;
use std::io;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::io::RawFd;
use std::path::Path;

#[derive(Debug)]
enum PhyData<'a> {
    Rx {
        buffer: crate::phy::BufferHandle<'a>,
        length: usize,
    },
    Tx {
        buffer: crate::phy::BufferHandle<'a>,
        length: usize,
        cursor: usize,
    },
}

impl PhyData<'_> {
    pub fn is_rx(&self) -> bool {
        match self {
            PhyData::Rx { .. } => true,
            _ => false,
        }
    }

    pub fn is_tx(&self) -> bool {
        match self {
            PhyData::Tx { .. } => true,
            _ => false,
        }
    }

    pub fn make_rx(&mut self) {
        if let PhyData::Tx { buffer, .. } = self {
            let buffer = std::mem::replace(buffer, [].into());
            *self = PhyData::Rx { buffer, length: 0 };
        }
    }
}

#[derive(Debug)]
pub struct LinuxRs485Phy<'a> {
    fd: RawFd,
    data: PhyData<'a>,
}

impl LinuxRs485Phy<'_> {
    #[inline]
    pub fn new<P: AsRef<Path>>(serial_port: P, baudrate: crate::Baudrate) -> Self {
        Self::new_inner(&serial_port.as_ref(), baudrate)
    }

    fn new_inner(serial_port: &Path, baudrate: crate::Baudrate) -> Self {
        // open serial port non-blocking
        let path = std::ffi::CString::new(serial_port.as_os_str().as_bytes()).unwrap();
        let fd = unsafe {
            libc::open(
                path.as_ptr() as *const i8,
                libc::O_RDWR | libc::O_NONBLOCK | libc::O_NOCTTY,
            )
        };
        if fd < 0 {
            let error = io::Error::last_os_error();
            Result::<(), _>::Err(error).unwrap();
        }

        let mut tty: libc::termios2 = unsafe { core::mem::zeroed() };
        if unsafe { libc::ioctl(fd, libc::TCGETS2, &mut tty) } < 0 {
            let error = io::Error::last_os_error();
            Result::<(), _>::Err(error).unwrap();
        }

        tty.c_iflag &= !(libc::IGNBRK
            | libc::BRKINT
            | libc::PARMRK
            | libc::ISTRIP
            | libc::INLCR
            | libc::IGNCR
            | libc::ICRNL
            | libc::IXON);
        tty.c_oflag &= !(libc::OPOST | libc::ONLCR);
        tty.c_lflag &= !(libc::ISIG
            | libc::ICANON
            | libc::IEXTEN
            | libc::ECHO
            | libc::ECHOE
            | libc::ECHOK
            | libc::ECHONL);

        tty.c_cflag &= !(libc::CSIZE
            | libc::PARODD
            | libc::CSTOPB
            | libc::CRTSCTS
            | (libc::CBAUD | libc::CBAUDEX)
            | ((libc::CBAUD | libc::CBAUDEX) << libc::IBSHIFT));
        tty.c_cflag |= libc::CS8 | libc::PARENB | libc::BOTHER | (libc::BOTHER << libc::IBSHIFT);

        // Ensure non-blocking access
        tty.c_cc[libc::VMIN] = 0;
        tty.c_cc[libc::VTIME] = 0;

        // Set speed
        let baud = baudrate.to_rate().try_into().unwrap();
        tty.c_ispeed = baud;
        tty.c_ospeed = baud;
        log::debug!("Speed: {}", tty.c_ispeed);

        if unsafe { libc::ioctl(fd, libc::TCSETS2, &tty) } < 0 {
            let error = io::Error::last_os_error();
            Result::<(), _>::Err(error).unwrap();
        }

        // Read back to ensure baudrates are correct.
        if unsafe { libc::ioctl(fd, libc::TCGETS2, &mut tty) } < 0 {
            let error = io::Error::last_os_error();
            Result::<(), _>::Err(error).unwrap();
        }

        assert_eq!(
            tty.c_ispeed, baud,
            "c_ispeed not matching expected baudrate"
        );
        assert_eq!(
            tty.c_ospeed, baud,
            "c_ospeed not matching expected baudrate"
        );

        let res = rs485::SerialRs485::new()
            .set_enabled(true)
            .set_rts_on_send(true)
            .set_rts_after_send(false)
            .set_rx_during_tx(false)
            .set_on_fd(fd);
        if let Err(e) = res {
            log::warn!("Could not configure RS485 mode: {}", e);
        }

        // TODO: Allow configuring this buffer?
        let buffer = crate::phy::BufferHandle::from(vec![0u8; 512]);

        Self {
            fd,
            data: PhyData::Rx { buffer, length: 0 },
        }
    }

    /// Wait/block until the current transmission completes.
    ///
    /// This is useful to save CPU time as the PROFIBUS stack can't do much anyway until the
    /// transmission is over.
    pub fn wait_transmit(&mut self) {
        if self.data.is_tx() {
            unsafe { libc::tcdrain(self.fd) };
        }
    }

    fn write(fd: RawFd, buffer: &[u8]) -> io::Result<usize> {
        match unsafe { libc::write(fd, buffer.as_ptr() as *const c_void, buffer.len()) } {
            -1 => {
                let err = io::Error::last_os_error();
                if err.kind() == io::ErrorKind::WouldBlock {
                    Ok(0)
                } else {
                    Err(err)
                }
            }
            written => Ok(written as usize),
        }
    }

    fn get_output_queue(&mut self) -> io::Result<usize> {
        let mut arg: std::ffi::c_int = 0;
        let res = unsafe { libc::ioctl(self.fd, libc::TIOCOUTQ, &mut arg) };
        if res < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(arg as usize)
    }

    fn read(fd: RawFd, buffer: &mut [u8]) -> io::Result<usize> {
        match unsafe { libc::read(fd, buffer.as_mut_ptr() as *mut c_void, buffer.len()) } {
            -1 => {
                let err = io::Error::last_os_error();
                if err.kind() == io::ErrorKind::WouldBlock {
                    Ok(0)
                } else {
                    Err(err)
                }
            }
            written => Ok(written as usize),
        }
    }
}

impl<'a> crate::phy::ProfibusPhy for LinuxRs485Phy<'a> {
    fn is_transmitting(&mut self) -> bool {
        if let PhyData::Tx {
            buffer,
            length,
            cursor,
        } = &mut self.data
        {
            if length != cursor {
                // Need to submit more data.
                let written = Self::write(self.fd, &buffer[*cursor..*length]).unwrap();
                debug_assert!(written <= *length - *cursor);
                *cursor += written;
                false
            } else {
                // Everything was submitted already.
                let queued = self.get_output_queue().unwrap();
                if queued == 0 {
                    // All data was sent.
                    self.data.make_rx();
                    false
                } else {
                    // Still sending.
                    true
                }
            }
        } else {
            false
        }
    }

    fn transmit_data<F, R>(&mut self, f: F) -> R
    where
        F: FnOnce(&mut [u8]) -> (usize, R),
    {
        match &mut self.data {
            PhyData::Tx { .. } => panic!("transmit_data() while already transmitting!"),
            PhyData::Rx {
                buffer,
                length: receive_length,
            } => {
                if *receive_length != 0 {
                    log::warn!(
                        "{} bytes in the receive buffer and we go into transmission?",
                        receive_length
                    );
                }
                let (length, res) = f(&mut buffer[..]);
                if length == 0 {
                    // Don't transmit anything.
                    return res;
                }
                let cursor = Self::write(self.fd, &buffer[..length]).unwrap();
                debug_assert!(cursor <= length);
                let buffer = std::mem::replace(buffer, [].into());
                self.data = PhyData::Tx {
                    buffer,
                    length,
                    cursor,
                };
                res
            }
        }
    }

    fn receive_data<F, R>(&mut self, f: F) -> R
    where
        F: FnOnce(&[u8]) -> (usize, R),
    {
        match &mut self.data {
            PhyData::Tx { .. } => panic!("receive_data() while transmitting!"),
            PhyData::Rx { buffer, length } => {
                *length += Self::read(self.fd, &mut buffer[*length..]).unwrap();
                debug_assert!(*length <= buffer.len());
                let (drop, res) = f(&buffer[..*length]);
                match drop {
                    0 => (),
                    d if d == *length => *length = 0,
                    d => todo!("drop partial receive buffer ({} bytes of {})", d, *length),
                }
                res
            }
        }
    }
}
