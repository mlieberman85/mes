use crate::cpu::opcode::*;
use crate::cpu::opcode::DecodeError::IllegalUnimplementedOpcode;
use crate::cpu::opcode::Instruction::UNK;

const ROM_START: usize = 0x8000;
const ROM_END: usize =0xFFFF;

#[derive(Debug, Clone)]
pub enum ROMError {
    InvalidHeader {
        header_bytes: [u8; ROMHeader::HEADER_SIZE]
    }
}

pub struct ROM {
    header: ROMHeader,
    pub prg: Vec<u8>,
    chr: Vec<u8>
}

impl ROM {
    /// iNES format states that the minimum size of a rom is 16kb. There are a handful of games that
    /// are less than 16kb like Galaxian, but they are overdumps where there is junk data making up
    /// the difference. That data is just ignored by the emulator but requires by the iNES rom spec.
    const MINIMUM_ROM_SIZE: u16 = 16384;

    pub fn new(rom_bytes: Vec<u8>) -> Result<Self, ROMError> {
        let mut header_bytes: [u8; ROMHeader::HEADER_SIZE] = [0; ROMHeader::HEADER_SIZE];
        header_bytes.copy_from_slice(&rom_bytes[0..ROMHeader::HEADER_SIZE]);
        let header = ROMHeader::new(header_bytes)?;

        // 0x4000 is the "bank" size I think. I've seen the words "chunks" and "pages" also used
        // 0x4000 is 16kb.
        // For ease of reference 16kb is the size of the upper/lower rom banks. If ROM is only 16kb
        // then it is mirrored.
        let prg_end = header.prg_rom_start_offset() + (header.num_prg_banks * 0x4000);
        let chr_end = prg_end + header.num_chr_banks * (header.num_chr_banks * 0x2000);

        let prg = rom_bytes[header.prg_rom_start_offset()..prg_end].to_vec();
        let chr = rom_bytes[prg_end..chr_end].to_vec();

        Ok(ROM {
            header,
            prg,
            chr
        })
    }
}

/// This is the header for a ROM. It contains information for the following things:
/// * Whether or not this is a valid NES rom. e.g. if the rom doesn't start with "NES" it's not
///   valid.
/// * Number of program and character ROM banks
/// * Bits used to determine what mapper the ROM uses.
/// * Bits used to determine V or H mirroring.
struct ROMHeader {
    // First 4 bytes of header should be N E S in hex + "1A" which is a character break. Storing it
    // here for informational purposes.
    nes: [u8; 4],
    num_prg_banks: usize,
    num_chr_banks: usize,
    // Lower mapper byte also includes V or H mirroring, Battery, 4 screen VRAM and trainer switches
    // V or H mirroring is the only pertinent piece for this emulator right now.
    lower_mapper_bits: u8,
    upper_mapper_bits: u8,
    // Due to the NES rom spec there from byte 8 (assuming starting from 0) to byte 15 are just
    // zeros. Leaving that here also for informational purposes
    zeros: [u8; 8]
}

impl ROMHeader {
    const HEADER_SIZE: usize = 16;

    pub fn new(header_bytes: [u8; ROMHeader::HEADER_SIZE]) -> Result<Self, ROMError> {
        let mut nes: [u8; 4] = [0; 4];
        nes.copy_from_slice(&header_bytes[0..=3]);
        let expected_nes :[u8; 4] = [0x4E, 0x45, 0x53, 0x1A];
        if nes != expected_nes {
            Err(ROMError::InvalidHeader { header_bytes })
        } else {
            let num_prg_banks = header_bytes[4] as usize;
            let num_chr_banks = header_bytes[5] as usize;
            // Lower mapper byte also includes V or H mirroring, Battery, 4 Screen VRAM and trainer switches
            let lower_mapper_bits = header_bytes[6];
            let upper_mapper_bits = header_bytes[7];
            let mut zeros: [u8; 8] = [0; 8];
            zeros.copy_from_slice(&header_bytes[8..=15]);

            // If true there's trainer switches. According to most sources trainers are no longer
            // really used however. This means we can just skip over them if they exist. If they exist
            // in the rom they're the 512 bytes after the header.
            let actual_rom_start_offset = {
                if lower_mapper_bits & 0x04 != 0 {
                    (ROMHeader::HEADER_SIZE + 512) as usize
                } else {
                    ROMHeader::HEADER_SIZE as usize
                }
            };

            Ok(ROMHeader { nes, num_prg_banks, num_chr_banks, lower_mapper_bits, upper_mapper_bits, zeros })
        }
    }

    pub fn prg_rom_start_offset(&self) -> usize {
        if self.lower_mapper_bits & 0x04 != 0 {
            (ROMHeader::HEADER_SIZE + 512) as usize
        }
        else {
            ROMHeader::HEADER_SIZE as usize
        }
    }

    pub fn mapper_id(&self) -> u8 {
        (self.lower_mapper_bits & 0xF0) >> 4 | self.upper_mapper_bits & 0xF0
    }
}

pub trait DisassembleRom {
    fn disassemble_prg_rom(&self) -> Result<String, DecodeError>;
}

impl DisassembleRom for ROM {
    /// Disassembles a rom into 6502 assembly. I assume this will fail on overdumped roms due to
    /// potential for junk data passed into the prg rom.
    /// Currently this is a naive way of disassembling. A better way is to actually step through
    /// instructions.
    /// This also currently uses *-/*+ relative addressing. A better way is to convert relative
    /// addresses to real addresses.
    fn disassemble_prg_rom(&self) -> Result<String, DecodeError> {
        let mut head: usize = 0;
        let mut disassembled = String::new();
        while head < self.prg.len() - 1 {
            disassembled.push_str(&format!("{:04X?}    ", head));
            let opcode = self.prg[head];
            let result = opcode.decode();
            let decoded_opcode: DecodedOpcode = match result {
                Err(IllegalUnimplementedOpcode { opcode }) => {
                    DecodedOpcode {
                        instruction: UNK,
                        mode: AddressingMode::Implied,
                        cycles: 0
                    }
                }
                Ok(decoded_opcode) => decoded_opcode
            };
            head += 1;
            // See the Addressing mode comments for what the operands look like disassembled.
            // Reminder: 6502 is little endian, so two byte operands are reversed when disassembled.
            let line = match decoded_opcode.mode {
                AddressingMode::ZeroPage => {
                    let temp = format!("{} ${:X?}", decoded_opcode.instruction.to_string(), self.prg[head]);
                    head += 1;
                    temp
                },
                AddressingMode::IndexedZeroPageX => {
                    let temp = format!("{} ${:X?},X", decoded_opcode.instruction.to_string(), self.prg[head]);
                    head += 1;
                    temp
                },
                AddressingMode::IndexedZeroPageY => {
                    let temp = format!("{} ${:X?},Y", decoded_opcode.instruction.to_string(), self.prg[head]);
                    head += 1;
                    temp
                },
                AddressingMode::Absolute => {
                    let temp = format!("{} ${:X?}{:X?}", decoded_opcode.instruction.to_string(), self.prg[head + 1], self.prg[head]);
                    head += 2;
                    temp
                },
                AddressingMode::IndexedAbsoluteX => {
                    let temp = format!("{} ${:X?}{:X?},X", decoded_opcode.instruction.to_string(), self.prg[head + 1], self.prg[head]);
                    head += 2;
                    temp
                },
                AddressingMode::IndexedAbsoluteY => {
                    let temp = format!("{} ${:X?}{:X?},Y", decoded_opcode.instruction.to_string(), self.prg[head + 1], self.prg[head]);
                    head += 2;
                    temp
                },
                AddressingMode::Indirect => {
                    let temp = format!("{} $({:X?}{:X?})", decoded_opcode.instruction.to_string(), self.prg[head + 1], self.prg[head]);
                    head += 2;
                    temp
                },
                AddressingMode::Implied => {
                    format!("{}", decoded_opcode.instruction.to_string())
                },
                AddressingMode::Accumulator => {
                    format!("{} A", decoded_opcode.instruction.to_string())
                },
                AddressingMode::Immediate => {
                    let temp = format!("{} #${:X?}", decoded_opcode.instruction.to_string(), self.prg[head]);
                    head += 1;
                    temp
                },
                AddressingMode::Relative => {
                    let temp = format!("{} *{}", decoded_opcode.instruction.to_string(), self.prg[head] as i8);
                    head += 1;
                    temp
                },
                AddressingMode::IndexedIndirect => {
                    let temp = format!("{} (${:X?}, X)", decoded_opcode.instruction.to_string(), self.prg[head]);
                    head += 1;
                    temp
                },
                AddressingMode::IndirectIndexed => {
                    let temp = format!("{} (${:X?}), Y", decoded_opcode.instruction.to_string(), self.prg[head]);
                    head += 1;
                    temp
                },
            };
            disassembled.push_str(&format!("{}\n", line));
        }

        Ok(disassembled)
    }
}