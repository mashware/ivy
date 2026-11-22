//! Bounded parameter snapshots; the memory provider owns the platform-specific reads.
#[cfg(windows)]
use super::arguments;
#[cfg(not(windows))]
use super::windows_arguments as arguments;
use std::collections::hash_map::DefaultHasher;
use std::hash::Hasher;
use std::io;
fn invalid() -> io::Error {
    io::ErrorKind::InvalidData.into()
}
pub(super) trait Memory {
    fn read(&self, address: usize, buffer: &mut [u8]) -> io::Result<()>;
}
fn descriptor(memory: &impl Memory, address: usize) -> io::Result<Descriptor> {
    let mut b = [0; 16];
    memory.read(address, &mut b)?;
    let len = u16::from_le_bytes([b[0], b[1]]) as usize;
    let max = u16::from_le_bytes([b[2], b[3]]) as usize;
    let pointer =
        usize::try_from(u64::from_le_bytes(b[8..16].try_into().unwrap())).map_err(|_| invalid())?;
    if len == 0
        || !len.is_multiple_of(2)
        || !max.is_multiple_of(2)
        || len > max
        || pointer == 0
        || !pointer.is_multiple_of(2)
        || pointer.checked_add(len).is_none()
    {
        return Err(invalid());
    }
    Ok(Descriptor { len, max, pointer })
}
#[derive(Clone, Copy, PartialEq, Eq)]
struct Descriptor {
    len: usize,
    max: usize,
    pointer: usize,
}
pub(super) struct Stream<'a, M> {
    reader: &'a M,
    descriptor: Descriptor,
    pos: usize,
    hash: DefaultHasher,
}
impl<M: Memory> arguments::Units for Stream<'_, M> {
    fn next_unit(&mut self) -> io::Result<Option<u16>> {
        if self.pos == self.descriptor.len {
            return Ok(None);
        }
        let mut b = [0; 2];
        self.reader.read(
            self.descriptor
                .pointer
                .checked_add(self.pos)
                .ok_or_else(invalid)?,
            &mut b,
        )?;
        let u = u16::from_le_bytes(b);
        if u == 0 {
            return Err(invalid());
        }
        self.pos += 2;
        self.hash.write(&b);
        Ok(Some(u))
    }
}
pub(super) fn read_verified<T, M: Memory>(
    memory: &M,
    address: usize,
    read: impl FnOnce(&mut Stream<'_, M>) -> io::Result<T>,
) -> io::Result<T> {
    let desc = descriptor(memory, address)?;
    let mut stream = Stream {
        reader: memory,
        descriptor: desc,
        pos: 0,
        hash: DefaultHasher::new(),
    };
    let value = read(&mut stream)?;
    let (count, hash) = (stream.pos, stream.hash.finish());
    let mut verify = Stream {
        reader: memory,
        descriptor: desc,
        pos: 0,
        hash: DefaultHasher::new(),
    };
    while verify.pos < count {
        arguments::Units::next_unit(&mut verify)?;
    }
    if verify.hash.finish() != hash || descriptor(memory, address)? != desc {
        return Err(invalid());
    }
    Ok(value)
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::{Cell, RefCell};
    struct Fake {
        bytes: RefCell<Vec<u8>>,
        reads: Cell<usize>,
        mutate_at: usize,
        mutate_offset: usize,
        short: bool,
    }
    impl Memory for Fake {
        fn read(&self, address: usize, out: &mut [u8]) -> io::Result<()> {
            let n = self.reads.get() + 1;
            self.reads.set(n);
            let mut bytes = self.bytes.borrow_mut();
            if n == self.mutate_at {
                bytes[self.mutate_offset] ^= 1;
            }
            if self.short {
                return Err(invalid());
            }
            let src = bytes
                .get(address..address.checked_add(out.len()).ok_or_else(invalid)?)
                .ok_or_else(invalid)?;
            out.copy_from_slice(src);
            Ok(())
        }
    }
    fn fake(text: &str) -> Fake {
        let data: Vec<_> = text.encode_utf16().flat_map(u16::to_le_bytes).collect();
        let mut bytes = vec![0; 64];
        bytes[8..10].copy_from_slice(&(data.len() as u16).to_le_bytes());
        bytes[10..12].copy_from_slice(&(data.len() as u16).to_le_bytes());
        bytes[16..24].copy_from_slice(&64u64.to_le_bytes());
        bytes.extend(data);
        Fake {
            bytes: RefCell::new(bytes),
            reads: Cell::new(0),
            mutate_at: usize::MAX,
            mutate_offset: 0,
            short: false,
        }
    }
    #[test]
    fn reads_unicode_units_through_the_production_stream() {
        let f = fake("猫😀");
        let units = read_verified(&f, 8, |stream| {
            let mut units = Vec::new();
            while let Some(unit) = arguments::Units::next_unit(stream)? {
                units.push(unit);
            }
            Ok(units)
        })
        .unwrap();
        assert_eq!(String::from_utf16(&units).unwrap(), "猫😀");
    }
    #[test]
    fn rejects_bad_descriptors_and_short_reads() {
        for offset in [8, 10, 16] {
            let f = fake("abc");
            f.bytes.borrow_mut()[offset] |= 1;
            assert_eq!(
                descriptor(&f, 8).err().unwrap().kind(),
                io::ErrorKind::InvalidData
            );
        }
        let mut f = fake("abc");
        f.short = true;
        assert_eq!(
            descriptor(&f, 8).err().unwrap().kind(),
            io::ErrorKind::InvalidData
        );
    }
    #[test]
    fn rejects_length_beyond_capacity_and_null_buffer() {
        let f = fake("abc");
        f.bytes.borrow_mut()[8..10].copy_from_slice(&8u16.to_le_bytes());
        assert!(descriptor(&f, 8).is_err());
        let f = fake("abc");
        f.bytes.borrow_mut()[16..24].copy_from_slice(&0u64.to_le_bytes());
        assert!(descriptor(&f, 8).is_err());
    }

    #[test]
    fn detects_changed_content_and_descriptor() {
        for (at, offset) in [(4, 64), (6, 8)] {
            let mut f = fake("ab");
            f.mutate_at = at;
            f.mutate_offset = offset;
            let result = read_verified(&f, 8, |s| {
                while arguments::Units::next_unit(s)?.is_some() {}
                Ok(())
            });
            assert_eq!(result.unwrap_err().kind(), io::ErrorKind::InvalidData);
        }
    }
    #[test]
    fn mismatch_does_not_read_later_payload_even_during_revalidation() {
        let f = fake("x --bad payload");
        let shape = [crate::foreground::Argument::Literal("--good")];
        assert!(!read_verified(&f, 8, |s| arguments::matches(s, &[&shape])).unwrap());
        // Each consumed unit is read twice, plus two descriptor reads.
        assert!(f.reads.get() < 2 + 2 * "x --bad payload".len());
    }
}
