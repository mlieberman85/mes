use crate::cpu::opcode::*;
use crate::bus::bus::*;
use std::fmt;
use StatusFlags::*;
use std::convert::TryInto;
use crate::cpu::opcode::AddressingMode::*;


pub struct CPU {
    // Accumulator
    a: u8,

    // Index Registers
    x: u8,
    y: u8,

    // Program Counter
    pub pc: u16,

    // Stack Pointer
    sp: u8,

    // Status Register
    p: u8, // Only 6 bits needed

    pub bus: Bus,

    cycles: u8,

    pub current_instruction: u8,

    pub total_cycles: u32,

    pub current_opcode: DecodedOpcode,

    current_fetched_word: u16,
}

impl fmt::Debug for CPU {
    /// Custom implementation intended to format similarly to: nestest.log
    /// See: http://www.qmtpro.com/~nes/misc/nestest.log for example.
    /// Example line below:
    /// C000  4C F5 C5  JMP $C5F5                       A:00 X:00 Y:00 P:24 SP:FD PPU:  0,  0 CYC:7
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        write!(f, "{:X?}  {:X?} {:X?} {:X?}  {:?} {:28X?}     A:{:02X?} X:{:02X?} Y:{:02X?} P:{:02X?} SP:{:02X?} PPU:{:3} {}  CYC:{}",
               self.pc,
               self.current_instruction,
               self.current_fetched_word,  // This should be the second byte
               self.current_fetched_word,  // This should be the first byte
               self.current_opcode.instruction,
               self.current_fetched_word,
               self.a,
               self.x,
               self.y,
               self.p,
               self.sp,
               "0",  // This will be ppu
               "0", // Don't know what this is lol
               self.total_cycles
        )
    }
}

impl CPU {
    pub fn new(rom_vector: Vec<u8>) -> CPU {
        CPU {
            a: 0x00,
            x: 0x00,
            y: 0x00,
            // NOTE: Change below to 0xC000 when testing with nestest.nes rom and 0x8000 otherwise
            pc: 0xC000,
            sp: 0xFD,
            p: 0x24,
            // TODO: Fix error handlings
            bus: Bus::new(rom_vector).unwrap_or_else(|_| { panic!("Unable to load rom") }),
            cycles: 0,
            current_instruction: 0,  // Useful for debugging
            total_cycles: 7, // CPU takes 7 cycles to boot up.
            current_opcode: DecodedOpcode {
                instruction: Instruction::NOP,
                mode: AddressingMode::Absolute,
                cycles: 0,
            },
            current_fetched_word: 0x0000,
        }
    }

    fn get_status(&self, flag: StatusFlags) -> bool { (self.p & (flag as u8)) > 0 }

    fn set_status(&mut self, flag: StatusFlags, state: bool) {
        if state { self.p |= (flag as u8) } else { self.p &= !(flag as u8) }
    }

    pub fn debug_clock(&mut self) -> String {
        let debug = if self.cycles == 0 {
            let opcode = self.load_instruction().unwrap_or_else(|_| {
                // TODO: put this error handling elsewhere
                panic!("Invalid opcode!")
            });
            let debug = format!("{:X?}", self);
            self.pc += 1;
            self.cycles += self.execute(opcode);
            //self.execute(opcode);
            self.set_status(B, true); // This flag is unused but for accuracy should always be used

            debug
        } else {
            format!("{:X?}", self)
        };
        self.cycles -= 1;
        self.total_cycles += 1;

        debug
    }

    pub fn load_instruction(&mut self) -> Result<DecodedOpcode, DecodeError> {
        let instruction = self.bus.read(self.pc) as u8;
        let opcode = Opcode::decode(&instruction)?;
        self.set_status(B, true); // This flag is unused but for accuracy should always be used
        self.current_instruction = instruction;
        self.current_opcode = opcode.clone();
        self.cycles = opcode.cycles;

        Ok(opcode)
    }

    /// This handles the fetching, decoding and execution of an instruction. It also simulates
    /// the creation of
    pub fn clock(&mut self) {
        if self.cycles == 0 {
            let opcode = self.load_instruction().unwrap_or_else(|_| {
                // TODO: put this error handling elsewhere
                panic!("Invalid opcode!")
            });
            self.pc += 1;
            self.cycles += self.execute(opcode);
            //self.execute(opcode);
            self.set_status(B, true); // This flag is unused but for accuracy should always be used
        }
        self.cycles -= 1;
        self.total_cycles += 1;
    }

    pub fn reset(&mut self) {
        self.current_fetched_word = 0xFFFC; // This is the start address for that is read from memory
        let lo = self.bus.read(self.current_fetched_word);
        let hi = self.bus.read(self.current_fetched_word + 1);

        self.pc = ((hi as u16) << 8) | lo as u16;

        self.a = 0;
        self.x = 0;
        self.y = 0;
        self.sp = 0xFD;
        self.p = 0x00 | B as u8;  // FIXME: is U needed?

        self.cycles = 8;
    }

    fn irq(&mut self) {
        if self.get_status(I) == false {  // i.e. if interrupts are allowed
            self.bus.write(0x0100 + (self.sp as u16), (self.pc >> 8) as u8);
            self.bus.write(0x0100 + (self.sp - 1) as u16, (self.pc & 0x00FF) as u8);
            self.sp -= 2;

            self.set_status(B, false);
            self.set_status(B, true);
            self.set_status(I, true);
            self.bus.write(0x0100 + (self.sp as u16), self.p);
            self.sp -= 1;

            self.current_fetched_word = 0xFFFE;
            let lo = self.bus.read(self.current_fetched_word);
            let hi = self.bus.read(self.current_fetched_word + 1);
            self.pc = ((hi as u16) << 8) | lo as u16;

            self.cycles = 7;
        }
    }

    fn nmi(&mut self) {
        self.bus.write(0x0100 + (self.sp as u16), (self.pc >> 8) as u8);
        self.bus.write(0x0100 + (self.sp - 1) as u16, (self.pc & 0x00FF) as u8);
        self.sp -= 2;

        self.set_status(B, false);
        self.set_status(B, true);
        self.set_status(I, true);
        self.bus.write(0x0100 + (self.sp as u16), self.p);
        self.sp -= 1;

        self.current_fetched_word = 0xFFFA;
        let lo = self.bus.read(self.current_fetched_word);
        let hi = self.bus.read(self.current_fetched_word + 1);
        self.pc = ((hi as u16) << 8) | lo as u16;

        self.cycles = 7;
    }

    // Returns number of extra cycles to be performed if crossing page boundary
    fn fetch(&mut self, am: AddressingMode) -> u8 {
        use AddressingMode::*;
        match am {
            ZeroPage => self.fetch_zero_page(),
            IndexedZeroPageX => self.fetch_zero_page_x(),
            IndexedZeroPageY => self.fetch_zero_page_y(),
            Absolute => self.fetch_absolute(),
            IndexedAbsoluteX => self.fetch_absolute_x_indexed(),
            IndexedAbsoluteY => self.fetch_absolute_y_indexed(),
            Indirect => self.fetch_indirect(),
            Implied => self.fetch_implied(),
            Accumulator => self.fetch_accumulator(),
            Immediate => self.fetch_immediate(),
            Relative => self.fetch_relative(),
            IndexedIndirect => self.fetch_indexed_indirect(),
            IndirectIndexed => self.fetch_indirect_indexed()
        }
    }

    fn fetch_accumulator(&mut self) -> u8 {
        self.current_fetched_word = self.a.try_into().unwrap();
        0
    }

    fn fetch_immediate(&mut self) -> u8 {
        self.current_fetched_word = self.bus.read(self.pc).into();
        self.pc += 1;
        0
    }

    fn fetch_zero_page(&mut self) -> u8 {
        self.current_fetched_word = self.bus.read(self.pc).into();
        self.pc += 1;
        self.current_fetched_word &= 0x00FF;
        0
    }

    fn fetch_zero_page_x(&mut self) -> u8 {
        self.current_fetched_word = self.bus.read(self.pc) as u16 + (self.x as u16);
        self.pc += 1;
        self.current_fetched_word &= 0x00FF;
        0
    }

    fn fetch_zero_page_y(&mut self) -> u8 {
        self.current_fetched_word = self.bus.read(self.pc) as u16 + (self.y as u16);
        self.pc += 1;
        self.current_fetched_word &= 0x00FF;
        0
    }

    fn fetch_absolute(&mut self) -> u8 {
        let lo = self.bus.read(self.pc);
        let hi = self.bus.read(self.pc + 1);
        self.pc += 2;
        self.current_fetched_word = ((hi as u16) << 8 | lo as u16);
        0
    }

    fn fetch_absolute_x_indexed(&mut self) -> u8 {
        let lo = self.bus.read(self.pc);
        let hi = self.bus.read(self.pc + 1);
        self.current_fetched_word = ((hi as u16) << 8 | lo as u16);
        let (temp, _) = self.current_fetched_word.overflowing_add(self.x as u16);
        self.current_fetched_word = temp;
        self.pc += 2;
        self._extra_cycles(self.current_fetched_word, hi)
    }

    fn fetch_absolute_y_indexed(&mut self) -> u8 {
        let lo = self.bus.read(self.pc);
        let hi = self.bus.read(self.pc + 1);
        self.current_fetched_word = ((hi as u16) << 8 | lo as u16);
        let (temp, _) = self.current_fetched_word.overflowing_add(self.y as u16);
        self.current_fetched_word = temp;
        self.pc += 2;
        self._extra_cycles(self.current_fetched_word, hi)
    }

    /// This addressing mode purposefully does the wrong thing due to an error in 6502 hardware.
    /// If lo byte is 0xFF then high byte crosses page boundary. This should cross into next page
    /// but instead the bug was that it wraps to the beginning of the existing page and fetches
    /// that byte.
    fn fetch_indirect(&mut self) -> u8 {
        let lo = self.bus.read(self.pc);
        let hi = self.bus.read(self.pc + 1);
        self.pc += 2;

        let pointer: u16 = ((hi as u16) << 8 | lo as u16).into();

        if lo == 0xFF {  // i.e. if about to cross page boundary emulate bug
            self.current_fetched_word = (self.bus.read(pointer & 0xFF00) as u16) << 8 | self.bus.read(pointer) as u16
        } else {
            self.current_fetched_word = (self.bus.read(pointer + 1) as u16) << 8 | self.bus.read(pointer) as u16
        }
        0
    }

    // FIXME: Is this right?
    fn fetch_implied(&mut self) -> u8 {
        // Implied means the argument is implied in the instruction and doesn't come from a memory
        // address.
        self.current_fetched_word = 0x0000;
        0
    }

    // FIXME: Is this right?
    fn fetch_relative(&mut self) -> u8 {
        self.current_fetched_word = self.bus.read(self.pc) as u16;
        self.pc += 1;

        if (self.current_fetched_word & 0x80) >= 1 {
            self.current_fetched_word |= 0xFF00;
        }

        0
    }

    /// AKA Indirect X
    fn fetch_indexed_indirect(&mut self) -> u8 {
        self.current_fetched_word = self.bus.read(self.pc) as u16;
        self.pc += 1;

        let lo = (self.bus.read(self.current_fetched_word + self.x as u16 & 0x00FF) as u16);
        let hi = self.bus.read(self.current_fetched_word + 1 + self.x as u16 & 0x00FF) as u16;
        let indirect_address = hi << 8 | lo;
        self.current_fetched_word = indirect_address;
        0
    }

    /// AKA Indirect Y
    fn fetch_indirect_indexed(&mut self) -> u8 {
        self.current_fetched_word = self.bus.read(self.pc).try_into().unwrap();
        self.pc += 1;

        let lo = self.bus.read(self.current_fetched_word & 0x00FF);
        let hi = self.bus.read((self.current_fetched_word + 1) & 0x00FF);
        self.current_fetched_word = ((hi as u16) << 8) | lo as u16;
        let (temp, _) = self.current_fetched_word.overflowing_add(self.y as u16);
        self.current_fetched_word = temp;

        self._extra_cycles(self.current_fetched_word, hi)
    }

    /// Helper for determining if a page boundary has been crossed and needs extra cycle
    fn _extra_cycles(&self, addr: u16, hi: u8) -> u8 {
        if addr & 0xFF00 != (hi as u16) << 8 { 1 } else { 0 }
    }

    /// Instruction functionality below here

    fn execute(&mut self, opcode: DecodedOpcode) -> u8 {
        //let mut cycles = opcode.cycles;
        // TODO: Below has the side effect of fetching and writing data to and from registers, memory, etc.
        // FIXME: The below should both just return bools based on additoinal cycle
        let address_page_cross_cycle = self.fetch(opcode.mode);
        // FIXME: Most instructions don't care about addressing mode. Only immediate and accumulator based instructions
        let potential_extra_instruction_cycle = self.run_instruction(opcode.instruction, opcode.mode);
        let additional_cycles = address_page_cross_cycle & potential_extra_instruction_cycle;

        additional_cycles
    }

    fn run_instruction(&mut self, instruction: Instruction, mode: AddressingMode) -> u8 {
        use Instruction::*;
        match instruction {
            ADC => self.add_with_carry(mode), // Add Memory to Accumulator with Carry
            AND => self.logical_and(mode), // "AND" Memory with Accumulator
            ASL => self.arithmetic_shift_left(mode), // Shift Left One Bit (Memory or Accumulator)

            BCC => self.branch_if_carry_clear(), // Branch on Carry Clear
            BCS => self.branch_if_carry_set(), // Branch on Carry Set
            BEQ => self.branch_if_equal(), // Branch on Result Zero
            BIT => self.bit_test(), // Test Bits in Memory with Accumulator
            BMI => self.branch_if_minus(), // Branch on Result Minus
            BNE => self.branch_if_not_equal(), // Branch on Result not Zero
            BPL => self.branch_if_positive(), // Branch on Result Plus
            BRK => self.force_interrupt(), // Force Break
            BVC => self.branch_if_overflow_clear(), // Branch on Overflow Clear
            BVS => self.branch_if_overflow_set(), // Branch on Overflow Set

            CLC => self.clear_carry_flag(), // Clear Carry Flag
            CLD => self.clear_decimal_mode(), // Clear Decimal Mode
            CLI => self.clear_interrupt_disable(), // Clear interrupt Disable Bit
            CLV => self.clear_overflow_flag(), // Clear Overflow Flag
            CMP => self.compare(mode), // Compare Memory and Accumulator
            CPX => self.compare_x_register(mode), // Compare Memory and Index X
            CPY => self.compare_y_register(mode), // Compare Memory and Index Y

            DEC => self.decrement_memory(), // Decrement Memory by One
            DEX => self.decrement_x_register(), // Decrement Index X by One
            DEY => self.decrement_y_register(), // Decrement Index Y by One

            EOR => self.exclusive_or(mode), // "ExclusiveOr" Memory with Accumulator

            INC => self.increment_memory(), // Increment Memory by One
            INX => self.increment_x_register(), // Increment Index X by One
            INY => self.increment_y_register(), // Increment Index Y by One

            JMP => self.jump(), // Jump to New Location

            JSR => self.jump_to_subroutine(), // Jump to New Location Saving Return Address

            LDA => self.load_accumulator(mode), // Load Accumulator with Memory
            LDX => self.load_x_register(mode), // Load Index X with Memory
            LDY => self.load_y_register(mode), // Load Index Y with Memory
            LSR => self.logical_shift_right(mode), // Shift Right One Bit (Memory or Accumulator)

            NOP => self.no_operation(), // No Operation

            ORA => self.logical_inclusive_or(mode), // "OR" Memory with Accumulator

            PHA => self.push_accumulator(), // Push Accumulator on Stack
            PHP => self.push_processor_status(), // Push Processor Status on Stack
            PLA => self.pull_accumulator(), // Pull Accumulator from Stack
            PLP => self.pull_processor_status(), // Pull Processor Status from Stack

            ROL => self.rotate_left(mode), // Rotate One Bit Left (Memory or Accumulator)
            ROR => self.rotate_right(mode), // Rotate One Bit Right (Memory or Accumulator)
            RTI => self.return_from_interrupt(), // Return from Interrupt
            RTS => self.return_from_subroutine(), // Return from Subroutine

            SBC => self.subtract_with_carry(), // Subtract Memory from Accumulator with Borrow
            SEC => self.set_carry_flag(), // Set Carry Flag
            SED => self.set_decimal_flag(), // Set Decimal Mode
            SEI => self.set_interrupt_disable(), // Set Interrupt Disable Status
            STA => self.store_accumulator(), // Store Accumulator in Memory
            STX => self.store_x_register(), // Store Index X in Memory
            STY => self.store_y_register(), // Store Index Y in Memory

            TAX => self.transfer_accumulator_to_x(), // Transfer Accumulator to Index X
            TAY => self.transfer_accumulator_to_y(), // Transfer Accumulator to Index Y
            TSX => self.transfer_stack_pointer_to_x(), // Transfer Stack Pointer to Index X
            TXA => self.transfer_x_to_accumulator(), // Transfer Index X to Accumulator
            TXS => self.transfer_x_to_stack_pointer(), // Transfer Index X to Stack Pointer
            TYA => self.transfer_y_to_accumulator(), // Transfer Index Y to Accumulator

            // Below are all invalid aka unofficial instructions
            ALR => self.alr(),
            ANC => self.anc(),
            ARR => self.arr(),
            AXS => self.axs(),
            LAX => self.lax(),
            SAX => self.sax(),
            DCP => self.dcp(),
            ISC => self.isc(),
            RLA => self.rla(),
            RRA => self.rra(),
            SLO => self.slo(),
            SRE => self.sre(),

            // Below is needed for exhaustive matching but UNK is only really used when
            // disassembling a ROM
            UNK => unreachable!("UNK is here for debugging and decompiling")
        }
    }

    /// Most of the below instructions are based on the implementations as described in the
    /// following site: http://obelisk.me.uk/6502/reference.html. Other sites used for reference:
    ///
    /// http://wiki.nesdev.com/w/index.php/Nesdev_Wiki
    /// http://users.telenet.be/kim1-6502/6502/proman.html
    ///
    /// Also where descriptions weren't clear I did take a look at other's implementations,
    /// particularly:
    ///
    /// https://github.com/OneLoneCoder/olcNES/
    /// https://github.com/bokuweb/rustynes/
    ///
    /// A lot of binary math is abstracted from the bulk of the code. Even though some of the math
    /// is fairly simple, I often have typos and forget if I'm checking if a bit is set or unset in
    /// a given operation so I've written helper traits and functions.
    ///
    /// TODO: Write helpers like:
    ///     pop from stack
    ///     setting status flags based on common patterns
    ///
    /// TODO: Cleanup math and casting. It might be helpful to put some common operations that
    ///     involve a lot of casting into functions that return the correctly casted values.

    // FIXME: There's a lot of copy paste below, there's probably some things that can be done.

    /// FIXME: Below is incorrect, working around it by passing the addressing mode to some of the
    ///     Instructions. Need to figure out a better way of handling this though.
    fn fetch_operand(&mut self) -> u8 {
        let value = match self.current_opcode.mode {
            Immediate => self.current_fetched_word as u8,
            ZeroPage => self.bus.read(self.current_fetched_word),
            Absolute => self.bus.read(self.current_fetched_word),
            Relative => (self.current_fetched_word & 0xFF) as u8,
            Accumulator => self.current_fetched_word as u8,
            IndexedIndirect => (((self.bus.read(self.current_fetched_word + 1) as u16) << 8 | self.bus.read(self.current_fetched_word) as u16) & 0xFF) as u8,
            _ => self.bus.read(self.current_fetched_word)
        };

        value
    }

    /// TODO: This should include basic human understandable instructions on what each instruction
    ///     is doing.

    /// FIXME: There's no need to send the entire addressing mode, but it just makes the following
    /// code a bit simpler than creating a bool and doing the logic elsewhere.
    fn add_with_carry(&mut self, mode: AddressingMode) -> u8 {
        let operand = self.fetch_operand();

        let sum = (self.a as u16) + (operand as u16) + (self.get_status(C) as u16);
        self.set_status(C, sum > 0xFF);
        self.set_status(Z, (sum & 0x00FF) == 0);
        let overflow = (!(self.a ^ operand) as u16 & (self.a as u16 ^ sum) & 0x0080 != 0);
        self.set_status(V, overflow);
        self.set_status(N, (sum & 0x80) != 0);
        self.a = (sum & 0x00FF) as u8;

        1
    }

    fn logical_and(&mut self, mode: AddressingMode) -> u8 {
        self.a = self.a & match mode {
            _ => self.fetch_operand()
        };
        self.set_status(Z, self.a == 0x00);
        self.set_status(N, (self.a & 0b10000000) != 0);

        1
    }

    // TODO: Clean below up.
    fn arithmetic_shift_left(&mut self, mode: AddressingMode) -> u8 {
        let operand = match mode {
            Accumulator => self.a,
            _ => self.bus.read(self.current_fetched_word)
        };

        let shifted = (operand as u16) << 1;
        self.set_status(C, (shifted & 0xFF00) > 0);
        self.set_status(Z, (shifted & 0x00FF) == 0);
        self.set_status(N, (shifted & 0b10000000) != 0);

        match mode {
            Accumulator => self.a = shifted as u8,
            _ => self.bus.write(self.current_fetched_word, shifted as u8)
        };

        0
    }

    fn _branch_helper(&mut self) {
        self.cycles += 1;
        let branch_address = (self.pc as i16 + self.current_fetched_word as i16) as u16;
        if (branch_address & 0xFF00) != (self.pc & 0xFF00) {
            self.cycles += 1;
        }

        self.pc = branch_address
    }

    fn branch_if_carry_clear(&mut self) -> u8 {
        if !self.get_status(C) {
            self._branch_helper();
        }

        0
    }

    fn branch_if_carry_set(&mut self) -> u8 {
        if self.get_status(C) {
            self._branch_helper();
        }

        0
    }

    fn branch_if_equal(&mut self) -> u8 {
        if self.get_status(Z) {
            self._branch_helper();
        }

        0
    }

    fn bit_test(&mut self) -> u8 {
        let operand = self.fetch_operand();
        let test = self.a & operand;

        self.set_status(Z, (test & 0xFF) == 0);
        self.set_status(N, operand & (1 << 7) != 0);
        self.set_status(V, operand & (1 << 6) != 0);
        0
    }

    fn branch_if_minus(&mut self) -> u8 {
        if self.get_status(N) {
            self._branch_helper();
        }

        0
    }

    fn branch_if_not_equal(&mut self) -> u8 {
        if !self.get_status(Z) {
            self._branch_helper();
        }

        0
    }

    fn branch_if_positive(&mut self) -> u8 {
        if !self.get_status(N) {
            self._branch_helper();
        }

        0
    }

    // FIXME: Check if below is correct. Different docs indicate different implementations, particularly
    // around the state of the status flags and how they get pushed to stack.
    /// This should advance the program counter by 2, push the pc and status (p) registers to
    /// stack, sets the I flag, and reloads PC from $FFFE-$FFFF
    fn force_interrupt(&mut self) -> u8 {
        self.pc += 1;
        self.set_status(I, true); // FIXME: Some docs say this isn't needed. Figure it out.
        self.bus.write(0x0100 + (self.sp as u16), ((self.pc >> 8) as u8) & 0x00FF);
        self.bus.write(0x0100 + ((self.sp - 1) as u16), (self.pc as u8) & 0x00FF);
        self.sp -= 2;
        self.set_status(B, true);  // I think this flag is only needed when pushing the status register to stack.
        self.bus.write(0x0100 + (self.sp as u16), self.p);
        self.sp -= 1;
        self.set_status(B, false);

        self.pc = (self.bus.read(0xFFFF) as u16) << 8 | self.bus.read(0xFFFE) as u16;

        0
    }

    fn branch_if_overflow_clear(&mut self) -> u8 {
        if !self.get_status(V) {
            self._branch_helper();
        }

        0
    }

    fn branch_if_overflow_set(&mut self) -> u8 {
        if self.get_status(V) {
            self._branch_helper();
        }

        0
    }

    fn clear_carry_flag(&mut self) -> u8 {
        self.set_status(C, false);

        0
    }

    /// This isn't available on NES' 6502. Still implementing it since it's trivial and allows for
    /// reuse of this code
    fn clear_decimal_mode(&mut self) -> u8 {
        self.set_status(D, false);

        0
    }

    fn clear_interrupt_disable(&mut self) -> u8 {
        self.set_status(I, false);

        0
    }

    fn clear_overflow_flag(&mut self) -> u8 {
        self.set_status(V, false);

        0
    }

    fn _compare_helper(&mut self, register_value: u8) {
        let operand = self.fetch_operand();
        let (temp_difference, _) = register_value.overflowing_sub(operand);

        self.set_status(C, register_value >= operand);
        self.set_status(Z, temp_difference == 0);
        self.set_status(N, (temp_difference & 0b10000000) != 0);  // FIXME: Is this correct?
    }

    fn compare(&mut self, mode: AddressingMode) -> u8 {
        self._compare_helper(self.a);

        1
    }

    fn compare_x_register(&mut self, mode: AddressingMode) -> u8 {
        self._compare_helper(self.x);

        1
    }

    fn compare_y_register(&mut self, mode: AddressingMode) -> u8 {
        self._compare_helper(self.y);

        1
    }

    // FIXME: Can I use this?
    fn _decrement_helper(&mut self, operand: u8) -> u8 {
        let new = operand - 1;

        self.set_status(Z, operand == 0);
        self.set_status(N, (operand & 0b10000000) != 0);

        new
    }

    fn decrement_memory(&mut self) -> u8 {
        let (operand, _) = self.fetch_operand().overflowing_sub(1);

        self.bus.write(self.current_fetched_word, operand);
        self.set_status(Z, operand == 0);
        self.set_status(N, (operand & 0b10000000) != 0);

        0
    }

    fn decrement_x_register(&mut self) -> u8 {
        let (temp, _) = self.x.overflowing_sub(1);
        self.x = temp;
        self.set_status(Z, self.x == 0);
        self.set_status(N, (self.x & 0b10000000) != 0);

        0
    }

    fn decrement_y_register(&mut self) -> u8 {
        let (temp, _) = self.y.overflowing_sub(1);
        self.y = temp;
        self.set_status(Z, self.y == 0);
        self.set_status(N, (self.y & 0b10000000) != 0);

        0
    }

    fn exclusive_or(&mut self, mode: AddressingMode) -> u8 {
        let operand = self.fetch_operand();
        self.a ^= operand;

        self.set_status(Z, self.a == 0x00);
        self.set_status(N, (self.a & 0b10000000) != 0);

        1
    }

    fn increment_memory(&mut self) -> u8 {
        let (operand, _) = self.fetch_operand().overflowing_add(1);
        self.bus.write(self.current_fetched_word, operand); // FIXME: Is this right?

        self.set_status(Z, operand == 0);
        self.set_status(N, (operand & 0b10000000) != 0);

        0
    }

    fn increment_x_register(&mut self) -> u8 {
        let (temp, _) = self.x.overflowing_add(1);
        self.x = temp;
        self.set_status(Z, self.x == 0);
        self.set_status(N, (self.x & 0b10000000) != 0);

        0
    }

    fn increment_y_register(&mut self) -> u8 {
        let (temp, _) = self.y.overflowing_add(1);
        self.y = temp;
        self.set_status(Z, self.y == 0);
        self.set_status(N, (self.y & 0b10000000) != 0);

        0
    }

    fn jump(&mut self) -> u8 {
        self.pc = self.current_fetched_word;

        0
    }

    /// This one is somewhat non-trivial. We go back one in the program counter, write the current
    /// PC to the stack and then jump to the address in currently_fetched_word
    fn jump_to_subroutine(&mut self) -> u8 {
        self.pc -= 1;
        self.bus.write(0x0100 + self.sp as u16, ((self.pc >> 8) & 0x00FF) as u8);
        self.bus.write(0x0100 + (self.sp - 1) as u16, (self.pc & 0x00FF) as u8);
        self.sp -= 2;

        self.pc = self.current_fetched_word;

        0
    }

    fn load_accumulator(&mut self, mode: AddressingMode) -> u8 {
        self.a = self.fetch_operand();

        self.set_status(Z, self.a == 0);
        self.set_status(N, (self.a & 0b10000000) != 0);

        1
    }

    fn load_x_register(&mut self, mode: AddressingMode) -> u8 {
        self.x = self.fetch_operand();

        self.set_status(Z, self.x == 0);
        self.set_status(N, (self.x & 0b10000000) != 0);

        1
    }

    fn load_y_register(&mut self, mode: AddressingMode) -> u8 {
        self.y = self.fetch_operand();

        self.set_status(Z, self.y == 0);
        self.set_status(N, (self.y & 0b10000000) != 0);

        1
    }

    /// Shifts all bits right by one position. The original 0th bit is put into carry, i.e. if 0th
    /// bit is set it sets C status to true. 7th bit becomes 0. Zero flag is set if results is 0.
    /// Negative flag is set to true if 7th bit of result is set.
    ///
    /// NOTE: The above "Negative flag is set to true if 7th bit of result is set" doesn't make
    /// sense to me since 7th bit will always be "0" as part of the operation of the instruction.
    /// Still including it as it's in a lot of the documentation online.
    /// TODO: See above note. Try and reconcile this with more information.
    fn logical_shift_right(&mut self, mode: AddressingMode) -> u8 {
        let mut operand = match mode {
            Accumulator => self.a,
            _ => self.fetch_operand()
        };

        self.set_status(C, (operand & 0b00000001) == 1);

        operand >>= 1;

        self.set_status(Z, operand == 0x00);
        self.set_status(N, (operand & 0b10000000) != 0);

        match mode {
            Accumulator => self.a = operand,
            _ => self.bus.write(self.current_fetched_word, operand)
        };

        0
    }

    /// Some NOPs are different based on unofficial opcodes. Not implementing any for now.
    fn no_operation(&mut self) -> u8 {
        1 // There is the potential for an extra cycle only in illegal aka unofficial opcodes.
    }

    fn logical_inclusive_or(&mut self, mode: AddressingMode) -> u8 {
        let operand = self.fetch_operand();

        self.a |= operand;

        self.set_status(Z, self.a == 0x00);
        self.set_status(N, (self.a & 0b10000000) != 0);

        1
    }

    fn push_accumulator(&mut self) -> u8 {
        self.bus.write(0x0100 + self.sp as u16, self.a);
        self.sp -= 1;

        0
    }

    fn push_processor_status(&mut self) -> u8 {
        /// The spec says to also make sure this flag is set as well, even though it isn't used.
        /// It seems like the only time this actually matter is if you pop this off the stack into
        /// the accumulator.
        self.set_status(U, true);
        self.bus.write(0x0100 + self.sp as u16, self.p);
        self.sp -= 1;
        self.set_status(U, false);


        0
    }

    fn pull_accumulator(&mut self) -> u8 {
        self.sp += 1;
        self.a = self.bus.read(0x0100 + self.sp as u16);
        self.set_status(Z, self.a == 0x00);
        self.set_status(N, (self.a & 0b10000000) != 0);

        0
    }

    fn pull_processor_status(&mut self) -> u8 {
        self.sp += 1;
        self.p = self.bus.read(0x0100 + self.sp as u16);
        self.set_status(U, false);

        0
    }

    fn rotate_left(&mut self, mode: AddressingMode) -> u8 {
        let operand = match mode {
            Accumulator => self.a,
            _ => self.fetch_operand()
        };

        let shifted = operand << 1 | if self.get_status(C) { 1 } else { 0 };

        // Sets carry if the left most bit of the operand is set.
        self.set_status(C, (operand & 0b10000000) != 0);
        self.set_status(Z, shifted == 0x00);
        self.set_status(N, (shifted & 0b10000000) != 0);

        match mode {
            Accumulator => self.a = shifted,
            _ => self.bus.write(self.current_fetched_word, shifted)
        };

        0
    }

    fn rotate_right(&mut self, mode: AddressingMode) -> u8 {
        let operand = match mode {
            Accumulator => self.a,
            _ => self.fetch_operand()
        };


        let shifted = operand >> 1 | if self.get_status(C) { 1 << 7 } else { 0 };

        // Sets carry if the right most bit of the operand is set.
        self.set_status(C, (operand & 0b00000001) == 1);
        self.set_status(Z, shifted == 0x00);
        self.set_status(N, (shifted & 0b10000000) != 0);

        match mode {
            Accumulator => self.a = shifted,
            _ => self.bus.write(self.current_fetched_word, shifted)
        };

        0
    }

    /// This pops status from the stack and then pops the program counter from the next portion of
    /// stack.
    fn return_from_interrupt(&mut self) -> u8 {
        self.sp += 1;
        self.p = self.bus.read(0x0100 + (self.sp as u16));

        self.sp += 1;
        self.pc = self.bus.read(0x0100 + (self.sp as u16)) as u16 | (self.bus.read(0x0100 + ((self.sp + 1) as u16)) as u16) << 8;
        self.sp += 1;

        0
    }

    /// This pulls the subroutine jump start point from stack. It then increments the PC to the next.
    fn return_from_subroutine(&mut self) -> u8 {
        self.sp += 1;
        self.pc = self.bus.read(0x0100 + (self.sp as u16)) as u16 | (self.bus.read(0x0100 + ((self.sp + 1) as u16)) as u16) << 8;
        self.pc += 1;
        self.sp += 1;

        0
    }

    fn subtract_with_carry(&mut self) -> u8 {
        let operand = self.fetch_operand();

        let difference = self.a as i16 - operand as i16 - if self.get_status(C) { 0 } else { 1 };

        // FIXME: I think I might need to do some additional magic for the "sign" bit.
        self.set_status(C, difference >= 0);
        self.set_status(Z, difference == 0);
        self.set_status(V, (((self.a ^ operand) & 0x80) != 0 && ((self.a ^ difference as u8) & 0x80) != 0));
        self.set_status(N, (difference & 0b10000000) != 0);

        self.a = difference as u8;

        1
    }

    fn set_carry_flag(&mut self) -> u8 {
        self.set_status(C, true);

        0
    }

    // Not used on NES, but simple enough to implement in case this is used for other emulations
    fn set_decimal_flag(&mut self) -> u8 {
        self.set_status(D, true);

        0
    }

    fn set_interrupt_disable(&mut self) -> u8 {
        self.set_status(I, true);

        0
    }

    fn store_accumulator(&mut self) -> u8 {
        self.bus.write(self.current_fetched_word, self.a);

        0
    }

    fn store_x_register(&mut self) -> u8 {
        self.bus.write(self.current_fetched_word, self.x);

        0
    }

    fn store_y_register(&mut self) -> u8 {
        self.bus.write(self.current_fetched_word, self.y);

        0
    }

    fn transfer_accumulator_to_x(&mut self) -> u8 {
        self.x = self.a;

        self.set_status(Z, self.x == 0);
        self.set_status(N, self.x.is_negative());

        0
    }

    fn transfer_accumulator_to_y(&mut self) -> u8 {
        self.y = self.a;

        self.set_status(Z, self.y == 0);
        self.set_status(N, self.y.is_negative());

        0
    }

    fn transfer_stack_pointer_to_x(&mut self) -> u8 {
        self.x = self.sp;

        self.set_status(Z, self.x == 0);
        self.set_status(N, self.x.is_negative());

        0
    }

    fn transfer_x_to_accumulator(&mut self) -> u8 {
        self.a = self.x;

        self.set_status(Z, self.a == 0);
        self.set_status(N, self.a.is_negative());

        0
    }

    fn transfer_x_to_stack_pointer(&mut self) -> u8 {
        self.sp = self.x;

        0
    }

    fn transfer_y_to_accumulator(&mut self) -> u8 {
        self.a = self.y;

        self.set_status(Z, self.a == 0);
        self.set_status(N, self.a.is_negative());

        0
    }

    /// Below are implementations of the illegal aka unofficial instructions.
    /// The names of the below aren't great and mostly just taken from:
    /// https://wiki.nesdev.com/w/index.php/Programming_with_unofficial_opcodes
    ///
    /// Since these are not official instructions their operations are technically undefined and
    /// don't have a common name.
    //ALR
    fn alr(&mut self) -> u8 {
        unimplemented!()
    }

    // ANC
    fn anc(&mut self) -> u8 {
        unimplemented!()
    }

    // ARR
    fn arr(&mut self) -> u8 {
        unimplemented!()
    }

    // AXS
    fn axs(&mut self) -> u8 {
        unimplemented!()
    }

    // LAX
    fn lax(&mut self) -> u8 {
        self.a = self.fetch_operand();
        self.x = self.a;

        self.set_status(Z, self.x == 0);
        self.set_status(N, self.x.is_negative());
        1
    }

    // SAX
    fn sax(&mut self) -> u8 {
        self.bus.write(self.current_fetched_word, self.a & self.x);

        0
    }

    // DCP
    fn dcp(&mut self) -> u8 {
        let (operand, _) = self.fetch_operand().overflowing_sub(1);

        self.bus.write(self.current_fetched_word, operand);
        self.set_status(Z, operand == 0);
        self.set_status(N, (operand & 0b10000000) != 0);

        self._compare_helper(self.a);

        0
    }

    // ISC
    fn isc(&mut self) -> u8 {
        self.increment_memory();
        self.subtract_with_carry();
        0
    }

    // RLA
    fn rla(&mut self) -> u8 {
        self.rotate_left(self.current_opcode.mode);
        self.logical_and(self.current_opcode.mode);
        0
    }

    // RRA
    fn rra(&mut self) -> u8 {
        self.rotate_right(self.current_opcode.mode);
        self.add_with_carry(self.current_opcode.mode);
        0
    }

    // SLO
    fn slo(&mut self) -> u8 {
        self.arithmetic_shift_left(self.current_opcode.mode);
        self.logical_inclusive_or(self.current_opcode.mode);
        0
    }

    // SRE
    fn sre(&mut self) -> u8 {
        self.logical_shift_right(self.current_opcode.mode);
        self.exclusive_or(self.current_opcode.mode);
        0
    }
}

#[repr(u8)]
enum StatusFlags {
    /// For ease of reference:
    /// NVssDIZC - Bits from left to right:
    ///     Negative, Overflow, unused but always set, unused and only set when pushed on stack, decimal, interrupt, zero, carry
    C = (1 << 0),
    // Carry
    Z = (1 << 1),
    // Zero
    I = (1 << 2),
    // Disable Interrupts
    D = (1 << 3),
    // Unused decimal mode for emulation
    U = (1 << 4),
    // the 4's bit isn't used. Technically part of the Break flag which also isn't used. This appears to only be set when pushing the status register to the stack...
    B = (1 << 5),
    // Break, unused.
    V = (1 << 6),
    // Overflow
    N = (1 << 7), // Negative
}

/// Some helper traits and structs below

/// Helper to determine if the unsigned integers, i.e. u8 are negative. More straight forward and
/// readable than casting back and forth.
trait Negative {
    fn is_negative(&self) -> bool;
}

impl Negative for u8 {
    fn is_negative(&self) -> bool {
        self & 0b10000000 != 0
    }
}

