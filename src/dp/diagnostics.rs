#[derive(Default, PartialEq, Eq)]
pub struct ExtendedDiagnostics<'a> {
    buffer: &'a mut [u8],
    length: usize,
}

impl<'a> ExtendedDiagnostics<'a> {
    /// Whether extended diagnostics are even collected.
    ///
    /// This will return `true` when a buffer for extended diagnostics exists.
    pub fn is_available(&self) -> bool {
        self.buffer.len() > 0
    }

    pub fn raw_diag_buffer(&self) -> Option<&[u8]> {
        if !self.is_available() {
            None
        } else {
            Some(&self.buffer[..self.length])
        }
    }

    pub fn iter_diag_blocks(&self) -> ExtDiagBlockIter<'_> {
        ExtDiagBlockIter {
            ext_diag: self,
            cursor: 0,
        }
    }

    pub(crate) fn from_buffer(buffer: &'a mut [u8]) -> Self {
        Self { buffer, length: 0 }
    }

    pub(crate) fn fill(&mut self, buf: &[u8]) -> bool {
        if self.buffer.len() == 0 {
            // No buffer for ext. diagnostics so we ignore them entirely.
            false
        } else if self.buffer.len() < buf.len() {
            log::warn!(
                "Buffer too small for received ext. diagnostics, ignoring. ({} < {})",
                self.buffer.len(),
                buf.len()
            );
            false
        } else {
            self.buffer[..buf.len()].copy_from_slice(buf);
            self.length = buf.len();
            true
        }
    }
}

impl<'a> core::fmt::Debug for ExtendedDiagnostics<'a> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let mut dbg_list = f.debug_list();
        if self.is_available() {
            // TODO: Debug impl should also somehow display invalid diagnostics data
            for block in self.iter_diag_blocks() {
                dbg_list.entry(&block);
            }
        }
        dbg_list.finish()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum ChannelDataType {
    Bit = 0b001,
    Bit2 = 0b010,
    Bit4 = 0b011,
    Byte = 0b100,
    Word = 0b101,
    DWord = 0b110,
    Invalid = 0b111,
}

impl ChannelDataType {
    fn from_diag_byte2(b: u8) -> Self {
        match b >> 5 {
            0b001 => ChannelDataType::Bit,
            0b010 => ChannelDataType::Bit2,
            0b011 => ChannelDataType::Bit4,
            0b100 => ChannelDataType::Byte,
            0b101 => ChannelDataType::Word,
            0b110 => ChannelDataType::DWord,
            _ => ChannelDataType::Invalid,
        }
    }

    fn into_diag_byte2(self) -> u8 {
        (self as u8) << 5
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum ChannelError {
    ShortCircuit = 1,
    UnderVoltage = 2,
    OverVoltage = 3,
    OverLoad = 4,
    OverTemperature = 5,
    LineBreak = 6,
    UpperLimitOvershoot = 7,
    LowerLimitUndershoot = 8,
    Error = 9,
    Reserved(u8),
    Vendor(u8),
}

impl ChannelError {
    fn from_diag_byte2(b: u8) -> Self {
        match b & 0x1f {
            1 => ChannelError::ShortCircuit,
            2 => ChannelError::UnderVoltage,
            3 => ChannelError::OverVoltage,
            4 => ChannelError::OverLoad,
            5 => ChannelError::OverTemperature,
            6 => ChannelError::LineBreak,
            7 => ChannelError::UpperLimitOvershoot,
            8 => ChannelError::LowerLimitUndershoot,
            9 => ChannelError::Error,
            v @ 16..=31 => ChannelError::Vendor(v),
            r @ _ => ChannelError::Reserved(r),
        }
    }

    fn into_diag_byte2(self) -> u8 {
        match self {
            ChannelError::ShortCircuit => 1,
            ChannelError::UnderVoltage => 2,
            ChannelError::OverVoltage => 3,
            ChannelError::OverLoad => 4,
            ChannelError::OverTemperature => 5,
            ChannelError::LineBreak => 6,
            ChannelError::UpperLimitOvershoot => 7,
            ChannelError::LowerLimitUndershoot => 8,
            ChannelError::Error => 9,
            ChannelError::Vendor(v) => v,
            ChannelError::Reserved(r) => r,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChannelDiagnostics {
    pub module: u8,
    pub channel: u8,
    pub input: bool,
    pub output: bool,
    pub dtype: ChannelDataType,
    pub error: ChannelError,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExtDiagBlock<'a> {
    Identifier(&'a bitvec::slice::BitSlice<u8>),
    Channel(ChannelDiagnostics),
    Device(&'a [u8]),
}

pub struct ExtDiagBlockIter<'a> {
    ext_diag: &'a ExtendedDiagnostics<'a>,
    cursor: usize,
}

impl<'a> Iterator for ExtDiagBlockIter<'a> {
    type Item = ExtDiagBlock<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let raw_buffer = self.ext_diag.raw_diag_buffer().unwrap();
        if self.cursor >= raw_buffer.len() {
            return None;
        }

        let remainder = &raw_buffer[self.cursor..];
        let header = remainder[0];
        match header >> 6 {
            // Identifier-based Diagnostics
            0b01 => {
                let length = usize::from(header & 0x3f);
                if remainder.len() < length {
                    log::warn!("Diagnostics cut off: {:?}", remainder);
                    self.cursor = raw_buffer.len();
                    return None;
                }

                self.cursor += length;
                Some(ExtDiagBlock::Identifier(
                    bitvec::slice::BitSlice::from_slice(&remainder[1..length]),
                ))
            }
            // Channel-based Diagnostics
            0b10 => {
                if remainder.len() < 3 {
                    log::warn!("Diagnostics cut off: {:?}", remainder);
                    self.cursor = raw_buffer.len();
                    return None;
                }

                self.cursor += 3;
                Some(ExtDiagBlock::Channel(ChannelDiagnostics {
                    module: remainder[0] & 0x3f,
                    channel: remainder[1] & 0x3f,
                    input: remainder[1] & 0x40 != 0,
                    output: remainder[1] & 0x80 != 0,
                    dtype: ChannelDataType::from_diag_byte2(remainder[2]),
                    error: ChannelError::from_diag_byte2(remainder[2]),
                }))
            }
            // Device-based Diagnostics
            0b00 => {
                let length = usize::from(header & 0x3f);
                if remainder.len() < length {
                    log::warn!("Diagnostics cut off: {:?}", remainder);
                    self.cursor = raw_buffer.len();
                    return None;
                }

                self.cursor += length;
                Some(ExtDiagBlock::Device(&remainder[1..length]))
            }
            // Reserved
            0b11 => {
                log::warn!("Unexpected ext diag block: {:?}", remainder);
                self.cursor = raw_buffer.len();
                None
            }
            _ => unreachable!(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_diag_byte2() {
        for b in 0..=255u8 {
            // Filter edge cases
            if b & 0xe0 == 0 {
                continue;
            }

            let err = ChannelError::from_diag_byte2(b);
            let dtype = ChannelDataType::from_diag_byte2(b);

            let b_again = err.into_diag_byte2() | dtype.into_diag_byte2();
            assert_eq!(b, b_again);
        }
    }

    #[test]
    fn test_diag_iter() {
        let mut buffer = [
            0x44, 0x00, 0x01, 0x00, 0x88, 0x41, 0x21, 0x04, 0x10, 0x20, 0x30,
        ];
        let ext_diag = ExtendedDiagnostics {
            length: buffer.len(),
            buffer: &mut buffer[..],
        };

        let blocks: Vec<ExtDiagBlock> = ext_diag.iter_diag_blocks().collect();

        if let ExtDiagBlock::Identifier(i) = &blocks[0] {
            assert!(i.get(8).unwrap());
            assert!(i.count_ones() == 1);
            assert_eq!(i.len(), 24);
        } else {
            panic!("wrong diag block 0 {:?}", blocks[0]);
        }

        if let ExtDiagBlock::Channel(c) = &blocks[1] {
            assert_eq!(
                c,
                &ChannelDiagnostics {
                    module: 8,
                    channel: 1,
                    input: true,
                    output: false,
                    dtype: ChannelDataType::Bit,
                    error: ChannelError::ShortCircuit
                }
            );
        } else {
            panic!("wrong diag block 1 {:?}", blocks[1]);
        }

        if let ExtDiagBlock::Device(d) = &blocks[2] {
            assert_eq!(d, &[0x10, 0x20, 0x30]);
        } else {
            panic!("wrong diag block 2 {:?}", blocks[2]);
        }

        assert_eq!(blocks.len(), 3);
    }

    #[test]
    fn test_diag_iter_invalid() {
        let mut buffer = [0x44, 0x00, 0x01, 0x00, 0xff, 0x12, 0x34];
        let ext_diag = ExtendedDiagnostics {
            length: buffer.len(),
            buffer: &mut buffer[..],
        };

        let blocks: Vec<ExtDiagBlock> = ext_diag.iter_diag_blocks().collect();

        if let ExtDiagBlock::Identifier(i) = &blocks[0] {
            assert!(i.get(8).unwrap());
            assert!(i.count_ones() == 1);
            assert_eq!(i.len(), 24);
        } else {
            panic!("wrong diag block 0 {:?}", blocks[0]);
        }

        assert_eq!(blocks.len(), 1);
    }

    #[test]
    fn test_diag_iter_short() {
        // Identifier-based
        let mut buffer = [0x48, 0x00, 0x01, 0x00];
        let ext_diag = ExtendedDiagnostics {
            length: buffer.len(),
            buffer: &mut buffer[..],
        };

        let blocks = ext_diag.iter_diag_blocks().count();
        assert_eq!(blocks, 0);

        // Channel-based
        let mut buffer = [0x88, 0x00];
        let ext_diag = ExtendedDiagnostics {
            length: buffer.len(),
            buffer: &mut buffer[..],
        };

        let blocks = ext_diag.iter_diag_blocks().count();
        assert_eq!(blocks, 0);

        // Device-based
        let mut buffer = [0x08, 0x00, 0x01, 0x00];
        let ext_diag = ExtendedDiagnostics {
            length: buffer.len(),
            buffer: &mut buffer[..],
        };

        let blocks = ext_diag.iter_diag_blocks().count();
        assert_eq!(blocks, 0);
    }
}
