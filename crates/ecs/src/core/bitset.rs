pub use fixedbitset::*;

pub struct AccessBitset {
    bits: FixedBitSet,
}

impl AccessBitset {
    pub fn new() -> Self {
        Self {
            bits: FixedBitSet::new(),
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            bits: FixedBitSet::with_capacity(capacity * 2),
        }
    }

    pub fn get(&self, index: usize) -> (bool, bool) {
        let index = index * 2;

        let read = self.bits[index];
        let write = self.bits[index + 1];
        (read, write)
    }

    pub fn set(&mut self, index: usize, value: bool) {
        let index = index * 2;
        self.bits.set(index, value);
    }

    /// Sets the read bit for the given index.
    /// Returns `true` if the read bit was successfully set, otherwise `false`.
    pub fn read(&mut self, index: usize) -> bool {
        if self.bits[index + 1] {
            return false;
        } else {
            self.bits.set(index, true);
            return true;
        }
    }

    /// Sets the write bit for the given index.
    /// Returns `true` if the write bit was successfully set, otherwise `false`.
    pub fn write(&mut self, index: usize) -> bool {
        let (read, write) = self.get(index);
        if read || write {
            return false;
        } else {
            self.bits.set(index + 1, true);
            return true;
        }
    }

    pub fn reads(&self, index: usize) -> bool {
        self.bits[index * 2]
    }

    pub fn writes(&self, index: usize) -> bool {
        self.bits[index * 2 + 1]
    }

    pub fn conflicts(&self, other: &AccessBitset) -> bool {
        for i in 0..self.len() {
            let (read, write) = self.get(i);
            let (other_read, other_write) = other.get(i);

            if ((read || write) && other_write) || (other_read && write) {
                return true;
            }
        }

        false
    }

    pub fn grow(&mut self, size: usize) {
        self.bits.grow(size * 2);
    }

    pub fn iter(&self) -> AccessBitsetIter {
        AccessBitsetIter {
            bits: self,
            index: 0,
        }
    }

    pub fn len(&self) -> usize {
        self.bits.len() / 2
    }
}

pub struct AccessBitsetIter<'a> {
    bits: &'a AccessBitset,
    index: usize,
}

impl<'a> Iterator for AccessBitsetIter<'a> {
    type Item = (bool, bool);

    fn next(&mut self) -> Option<Self::Item> {
        if self.index < self.bits.len() {
            let value = self.bits.get(self.index);
            self.index += 1;
            Some(value)
        } else {
            None
        }
    }
}
