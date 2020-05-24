const ROM_START: usize = 0x8000;
const ROM_END: usize =0xFFFF;

struct ROM {
    prg: Vec<u8>,
    chr: Vec<u8>
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

trait DisassembleRom {
    fn disassemble_prg_rom(&self) -> String;
}

impl DisassembleRom for ROM {
    fn disassemble_prg_rom(&self) -> String {
        unimplemented!()
    }
}