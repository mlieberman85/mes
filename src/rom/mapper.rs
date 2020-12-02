pub trait Mapper {
    /// Since there can be bank switching address, the memory of the ROM is actually greater than
    /// 16-bit, but depending on stuff like which bank you're currently on, these functions will
    /// convert the 16-bit address coming from the cpu bus to athe actual memory location emulated
    fn prg_conversion(&self, address: usize) -> usize;
    fn chr_conversion(&self, address: usize) -> usize;
}

/// Mapper 000 aka NROM
///
/// This is a simple rom mapping with no extra features.
pub(crate) struct Nrom {
    pub num_prg_banks: usize,
    pub num_chr_banks: usize
}

/// The NROM mapper worked with wither 16kb for the prg-rom or 32kb for prg-rom. If it was 16kb it
/// would mirror the two 16kb prg-roms across the entire prg space.
impl Mapper for Nrom {
    fn prg_conversion(&self, address: usize) -> usize {
        let actual_address = {
            if self.num_prg_banks > 1 {
                address
            } else {
                address % 0x4000
            }
        } as usize;

        actual_address
    }

    fn chr_conversion(&self, address: usize) -> usize {
        let actual_address = {
            if self.num_prg_banks > 1 {
                address
            } else {
                address % 0x2000
            }
        } as usize;

        actual_address
    }
}