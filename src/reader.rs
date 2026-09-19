use crate::error::Error;

/// A cursor over a byte slice that reads sfnt's big-endian integers and
/// refuses to read past the end instead of panicking.
pub(crate) struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    pub(crate) fn new(data: &'a [u8]) -> Self {
        Reader { data, pos: 0 }
    }

    fn take(&mut self, n: usize) -> Result<&'a [u8], Error> {
        let end = self.pos.checked_add(n).unwrap_or(usize::MAX);
        if end > self.data.len() {
            return Err(Error::TooShort {
                needed: end,
                available: self.data.len(),
            });
        }
        let slice = &self.data[self.pos..end];
        self.pos = end;
        Ok(slice)
    }

    pub(crate) fn u16(&mut self) -> Result<u16, Error> {
        let b = self.take(2)?;
        Ok(u16::from_be_bytes([b[0], b[1]]))
    }

    pub(crate) fn i16(&mut self) -> Result<i16, Error> {
        Ok(self.u16()? as i16)
    }

    pub(crate) fn u32(&mut self) -> Result<u32, Error> {
        let b = self.take(4)?;
        Ok(u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
    }

    pub(crate) fn tag(&mut self) -> Result<[u8; 4], Error> {
        let b = self.take(4)?;
        Ok([b[0], b[1], b[2], b[3]])
    }

    /// Advance past `n` bytes without interpreting them.
    pub(crate) fn skip(&mut self, n: usize) -> Result<(), Error> {
        self.take(n)?;
        Ok(())
    }
}
