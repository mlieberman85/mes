use crate::rom::rom::{ROM, ROMError};

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

pub struct Bus {
    ram: RAM,
    rom: ROM,
    io_registers: IORegisters
}

impl Bus {
    pub fn new(rom: Vec<u8>) -> Result<Bus, ROMError> { // TODO: Update the error handling here
        Ok(Bus {
            ram: RAM::new(),
            rom: ROM::new(rom)?,
            io_registers: IORegisters::new()
        })
    }

    /// This is just a helper function mapping of address to device.
    fn get_mapped_device_and_real_address(&mut self, address: usize) -> (&mut dyn BusDevice, usize) {
        match address {
            RAM_START..=RAM_END => (&mut self.ram, address),
            PPU_START..=PPU_END => unimplemented!(),
            APU_IO_START..=APU_IO_END => (&mut self.io_registers, address - APU_IO_START),
            TEST_MODE_START..=TEST_MODE_END => unimplemented!(),
            CARTRIDGE_START..=CARTRIDGE_END => (&mut self.rom, address - ROM_START), // FIXME: this shouldn't be hard coded
            _ => unreachable!()
        }
    }
}

impl MemoryMap for Bus {
    fn read(&mut self, address: u16) -> u8 {
        let address = address as usize;
        let (device, real_address) = self.get_mapped_device_and_real_address(address);
        device.read(real_address)
    }

    fn write(&mut self, address: u16, data: u8) -> () {
        let address = address as usize;
        let (device, real_address) = self.get_mapped_device_and_real_address(address);
        device.write(real_address, data)
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
    fn write(&mut self, address: u16, data: u8) -> ();
}

/// Read and write functions for an individual device on the bus. Params should be the literal
/// addresses of the memory of each device. It works in tandem with the MemoryMap.
trait BusDevice {
    fn read(&self, address: usize) -> u8;
    fn write(&mut self, address: usize, data: u8) -> ();
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

    fn write(&mut self, address: usize, data: u8) -> () {
        self.memory[address % RAM_SIZE] = data;
    }
}

impl BusDevice for ROM {
    fn read(&self, address: usize) -> u8 {
        self.prg[self.mapper.prg_conversion(address)]
    }

    fn write(&mut self, address: usize, data: u8) -> () {
        self.prg[self.mapper.prg_conversion(address)] = data
    }
}

/// IORegisters are mostly used for audio but also controller
struct IORegisters {
    // Below taken from https://wiki.nesdev.com/w/index.php/APU_registers

    // APU

    // 	Pulse 1 channel (write)
    // $4000	DDLC NNNN	Duty, loop envelope/disable length counter, constant volume, envelope period/volume
    // $4001	EPPP NSSS	Sweep unit: enabled, period, negative, shift count
    // $4002	LLLL LLLL	Timer low
    // $4003	LLLL LHHH	Length counter load, timer high (also resets duty and starts envelope)
    pulse_1: Pulse,

    // Pulse 2 channel (write)
    // $4004	DDLC NNNN	Duty, loop envelope/disable length counter, constant volume, envelope period/volume
    // $4005	EPPP NSSS	Sweep unit: enabled, period, negative, shift count
    // $4006	LLLL LLLL	Timer low
    // $4007	LLLL LHHH	Length counter load, timer high (also resets duty and starts envelope)
    pulse_2: Pulse,

    // Triangle channel (write)
    // $4008	CRRR RRRR	Length counter disable/linear counter control, linear counter reload value
    // $400A	LLLL LLLL	Timer low
    // $400B	LLLL LHHH	Length counter load, timer high (also reloads linear counter)
    triangle: Triangle,

    // Noise channel (write)
    // $400C	--LC NNNN	Loop envelope/disable length counter, constant volume, envelope period/volume
    // $400E	L--- PPPP	Loop noise, noise period
    // $400F	LLLL L---	Length counter load (also starts envelope)
    noise: Noise,

    // DMC channel (write)
    // $4010	IL-- FFFF	IRQ enable, loop sample, frequency index
    // $4011	-DDD DDDD	Direct load
    // $4012	AAAA AAAA	Sample address %11AAAAAA.AA000000
    // $4013	LLLL LLLL	Sample length %0000LLLL.LLLL0001
    dmc: DMC,

    // TODO: $4014 writes to OAMDATA on PPU but not sure if ever actually used. Figure out if needed

    // $4015	---D NT21	Control: DMC enable, length counter enables: noise, triangle, pulse 2, pulse 1 (write)
    // $4015	IF-D NT21	Status: DMC interrupt, frame interrupt, length counter status: noise, triangle, pulse 2, pulse 1 (read)
    control_status: u8,

    // $4017	SD-- ----	Frame counter: 5-frame sequence, disable frame interrupt (write)
    frame_counter: u8
}

impl IORegisters {
    fn new() -> IORegisters {
        IORegisters {
            pulse_1: Pulse::new(),
            pulse_2: Pulse::new(),
            triangle: Triangle::new(),
            noise: Noise::new(),
            dmc: DMC::new(),
            control_status: 0x00,
            frame_counter: 0x00
        }
    }
}

impl BusDevice for IORegisters {
    fn read(&self, address: usize) -> u8 {
        match address {
            0x15 => unimplemented!(),
            _ => unreachable!("Roms shouldn't read from other IO registers")
        }
    }

    fn write(&mut self, address: usize, data: u8) -> () {
        match address {
            // Pulse 1
            0x00 => self.pulse_1.vol = data,
            0x01 => self.pulse_1.sweep = data,
            0x02 => self.pulse_1.lo = data,
            0x03 => self.pulse_1.hi = data,

            // Pulse 2
            0x04 => self.pulse_2.vol = data,
            0x05 => self.pulse_2.sweep = data,
            0x06 => self.pulse_2.lo = data,
            0x07 => self.pulse_2.hi = data,

            // Triangle
            0x08 => self.triangle.linear = data,
            0x09 => {}, // Unused
            0x0A => self.triangle.lo = data,
            0x0B => self.triangle.hi = data,

            // Noise
            0x0C => self.noise.vol = data,
            0x0D => {}, // Unused
            0x0E => self.noise.lo = data,
            0x0F => self.noise.hi = data,

            // DMC
            0x10 => self.dmc.freq = data,
            0x11 => self.dmc.raw = data,
            0x12 => self.dmc.start = data,
            0x13 => self.dmc.len = data,

            0x14 => {}, // TODO: Unsure if needed. The spec says this writes to PPU OAMDATA

            0x15 => self.control_status = data,

            0x16 => {}, // TODO: Unsure if needed. It says it's for feedback to joysticks

            0x17 => self.frame_counter = data,

            _ => unreachable!()
        }
    }
}

// TODO: Design and implement interface for how APU turns into actual sound

/// Names of vars below based on: https://wiki.nesdev.com/w/index.php/2A03

/// Pulse aka Square wave
struct Pulse {
    // DDLC NNNN	Duty, loop envelope/disable length counter, constant volume, envelope period/volume
    vol: u8,

    // EPPP NSSS	Sweep unit: enabled, period, negative, shift count
    sweep: u8,

    // LLLL LLLL	Timer low
    lo: u8,

    // LLLL LHHH	Length counter load, timer high (also resets duty and starts envelope)
    hi: u8
}

impl Pulse {
    fn new() -> Pulse {
        Pulse {
            vol: 0x00,
            sweep: 0x00,
            lo: 0x00,
            hi: 0x00
        }
    }
}
struct Triangle {
    // CRRR RRRR	Length counter disable/linear counter control, linear counter reload value
    linear: u8,

    // LLLL LLLL	Timer low
    lo: u8,

    // LLLL LHHH	Length counter load, timer high (also reloads linear counter)
    hi: u8
}

impl Triangle {
    fn new() -> Triangle {
        Triangle {
            linear: 0x00,
            lo: 0x00,
            hi: 0x00
        }
    }
}

struct Noise {
    // --LC NNNN	Loop envelope/disable length counter, constant volume, envelope period/volume
    vol: u8,

    // L--- PPPP	Loop noise, noise period
    lo: u8,

    // LLLL L---	Length counter load (also starts envelope)
    hi: u8
}

impl Noise {
    fn new() -> Noise {
        Noise {
            vol: 0x00,
            lo: 0x00,
            hi: 0x00
        }
    }
}

struct DMC {
    // $4010	IL-- FFFF	IRQ enable, loop sample, frequency index
    freq: u8,

    // $4011	-DDD DDDD	Direct load
    raw: u8,

    // $4012	AAAA AAAA	Sample address %11AAAAAA.AA000000
    start: u8,


    // $4013	LLLL LLLL	Sample length %0000LLLL.LLLL0001
    len: u8
}

impl DMC {
    fn new() -> DMC {
        DMC {
            freq: 0x00,
            raw: 0x00,
            start: 0x00,
            len: 0x00
        }
    }
}