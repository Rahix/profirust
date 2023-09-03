use crate::dp::Peripheral;
use core::fmt;

/// Storage type that can hold one peripheral.
#[derive(Default)]
pub struct PeripheralStorage<'a> {
    inner: Option<Peripheral<'a>>,
}

/// Handle that can be used to obtain a peripheral from the DP master.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PeripheralHandle {
    index: u8,
    address: u8,
}

impl PeripheralHandle {
    #[inline(always)]
    pub fn address(self) -> u8 {
        self.address
    }
}

impl fmt::Display for PeripheralHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Station {}", self.address)
    }
}

pub struct PeripheralSet<'a> {
    peripherals: managed::ManagedSlice<'a, PeripheralStorage<'a>>,
}

impl<'a> PeripheralSet<'a> {
    pub fn new<S>(storage: S) -> Self
    where
        S: Into<managed::ManagedSlice<'a, PeripheralStorage<'a>>>,
    {
        Self {
            peripherals: storage.into(),
        }
    }

    /// Add a peripheral to the set, and return its handle.
    ///
    /// # Panics
    /// This function panics if the storage is fixed-size (not a `Vec`) and is full.
    pub fn add(&mut self, peripheral: Peripheral<'a>) -> PeripheralHandle {
        for (index, slot) in self.peripherals.iter_mut().enumerate() {
            if slot.inner.is_none() {
                let address = peripheral.address();
                slot.inner = Some(peripheral);
                return PeripheralHandle {
                    index: u8::try_from(index).unwrap(),
                    address,
                };
            }
        }

        match &mut self.peripherals {
            managed::ManagedSlice::Borrowed(_) => panic!("Adding peripheral to full PeripheralSet"),
            managed::ManagedSlice::Owned(peripherals) => {
                let address = peripheral.address();
                peripherals.push(PeripheralStorage {
                    inner: Some(peripheral),
                });
                PeripheralHandle {
                    index: (peripherals.len() - 1).try_into().unwrap(),
                    address,
                }
            }
        }
    }

    /// Get a peripheral from the set by its handle, as mutable.
    ///
    /// # Panics
    /// This function may panic if the handle does not belong to this peripheral set.
    pub fn get_mut(&mut self, handle: PeripheralHandle) -> &mut Peripheral<'a> {
        self.peripherals[usize::from(handle.index)]
            .inner
            .as_mut()
            .expect("Handle does not refer to a valid peripheral")
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = (PeripheralHandle, &mut Peripheral<'a>)> {
        self.peripherals
            .iter_mut()
            .enumerate()
            .filter_map(|(i, p)| {
                p.inner.as_mut().map(|p| {
                    (
                        PeripheralHandle {
                            index: u8::try_from(i).unwrap(),
                            address: p.address(),
                        },
                        p,
                    )
                })
            })
    }
}
