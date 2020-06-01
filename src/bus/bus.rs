use crate::rom::rom::ROM;

const ROM_START: usize = 0x8000;
const ROM_END: usize = 0xFFFF;
const RAM_START: usize = 0x0000;
const RAM_END: usize = 0x1FFF;
const PPU_START: usize = 0x2000;
const PPU_END: usize = 0x3FFF;
const APU_IO_START: usize = 0x4000;
const APU_IO_END: usize = 0x4017;
const TEST_MODE_START: usize = 0x4018;
const TEST_MODE_END: usize = 0x401F;
const CARTRIDGE_START: usize = 0x4020;
const CARTRIDGE_END: usize = 0xFFFF;

const RAM_SIZE: usize = 0x800; // i.e. 2kb.

#[derive(Debug, Clone)]
pub enum BusError {}

struct Bus {
    ram: RAM,
    rom: ROM
}

impl Bus {
    /// This is just a helper function mapping of address to device.
    fn get_mapped_device(&mut self, address: usize) -> &mut dyn BusDevice {
        match address {
            RAM_START..=RAM_END => &mut self.ram,
            PPU_START..=PPU_END => unimplemented!(),
            APU_IO_START..=APU_IO_END => unimplemented!(),
            TEST_MODE_START..=TEST_MODE_END => unimplemented!(),
            CARTRIDGE_START..=CARTRIDGE_END => &mut self.rom,
            _ => unreachable!()
        }
    }
}

impl MemoryMap for Bus {
    fn read(&mut self, address: u16) -> u8 {
        let address = address as usize;
        self.get_mapped_device(address).read(address)
    }

    fn write(&mut self, address: u16, data: u8) -> Result<(), BusError> {
        let address = address as usize;
        self.get_mapped_device(address).write(address, data)
    }
}

/// Read and write functions that are expected to go through memory mapping in order to read/write
/// to the correct memory mapped device.
///
/// The way this is intended to work is that something tries to write to a memory address on the
/// Bus. This address is then passed through the memory map to a device and the address itself is
/// converted to the literal address that the device on the bus can use.
///
/// e.g. A map containing two devices.
///      One from memory $00 -> $19 and another from $20-$FF.
///      Caller request address $A1. This calls the second device. The mapping in that second device
///      determines that $A1 is actually $21 in the actual device.
pub trait MemoryMap {
    fn read(&mut self, address: u16) -> u8;
    fn write(&mut self, address: u16, data: u8) -> Result<(), BusError>;
}

/// Read and write functions for an individual device on the bus. Params should be the literal
/// addresses of the memory of each device. It works in tandem with the MemoryMap.
trait BusDevice {
    fn read(&self, address: usize) -> u8;
    fn write(&mut self, address: usize, data: u8) -> Result<(), BusError>;
}

struct RAM {
    memory: [u8; RAM_SIZE]
}

impl RAM {
    pub fn new() -> Self {
        RAM {
            memory: [0; RAM_SIZE]
        }
    }
}

impl BusDevice for RAM {
    fn read(&self, address: usize) -> u8 {
        self.memory[address % RAM_SIZE]
    }

    fn write(&mut self, address: usize, data: u8) -> Result<(), BusError> {
        self.memory[address % RAM_SIZE] = data;
        Ok(())
    }
}

impl BusDevice for ROM {
    fn read(&self, address: usize) -> u8 {
        unimplemented!()
    }

    fn write(&mut self, address: usize, data: u8) -> Result<(), BusError> {
        unimplemented!()
    }
}