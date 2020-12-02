mod bus;
mod cpu;
mod rom;

use crate::rom::rom::*;
use std::cell::Cell;
use std::f64;
use std::panic;
use std::rc::Rc;
use wasm_bindgen::JsCast;
use wasm_bindgen::__rt::core::cell::RefCell;
use wasm_bindgen::prelude::*;
use web_sys::console;
use crate::cpu::cpu::CPU;
use web_sys::console::debug;

// When the `wee_alloc` feature is enabled, this uses `wee_alloc` as the global
// allocator.
//
// If you don't want to use `wee_alloc`, you can safely delete this.
#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

pub struct State {
    rom: Option<ROM>,
    cpu: Option<CPU>
}

impl State {
    pub fn new() -> Self {
        State { rom: None, cpu: None }
    }

    pub fn set_rom(&mut self, rom_bytes: Vec<u8>) -> Result<(), ROMError> {
        self.rom = Some(ROM::new(rom_bytes)?);
        Ok(())
    }

    pub fn set_cpu(&mut self, rom_bytes: Vec<u8>) -> Result<(), ROMError> {
        self.rom = Some(ROM::new(rom_bytes.clone())?);
        self.cpu = Some(CPU::new(rom_bytes));
        Ok(())
    }
}

// This is like the `main` function, except for JavaScript.
#[wasm_bindgen(start)]
pub fn main_js() -> Result<(), JsValue> {
    // This provides better error messages in debug mode.
    // It's disabled in release mode so it doesn't bloat up the file size.
    #[cfg(debug_assertions)]
    console_error_panic_hook::set_once();

    let mut state = Rc::new(RefCell::new(State::new()));

    let document = web_sys::window().unwrap().document().unwrap();
    let canvas = document
        .create_element("canvas")?
        .dyn_into::<web_sys::HtmlCanvasElement>()?;
    document.body().unwrap().append_child(&canvas)?;

    let file_selector = document.create_element("input")?;

    file_selector.set_attribute("type", "file")?;
    file_selector.set_attribute("id", "rom-selector")?;
    file_selector.set_attribute("accept", ".nes")?;

    document.body().unwrap().append_child(&file_selector)?;

    let disassembler_output_div = Rc::new(RefCell::new(document.create_element("pre")?));
    disassembler_output_div
        .borrow_mut()
        .set_attribute("id", "disassembler-output")?;

    let debug_output_div = Rc::new(RefCell::new(document.create_element("pre")?));
    debug_output_div
        .borrow_mut()
        .set_attribute("id", "debug-output")?;

    document
        .body()
        .unwrap()
        .append_child(&disassembler_output_div.borrow())?;

    document
        .body()
        .unwrap()
        .append_child(&debug_output_div.borrow())?;

    let rom_selector: web_sys::HtmlInputElement = document
        .get_element_by_id("rom-selector")
        .unwrap()
        .dyn_into::<web_sys::HtmlInputElement>()?;
    {
        let file_reader = web_sys::FileReader::new()?;
        let closure = Closure::wrap(Box::new(move |event: web_sys::InputEvent| {
            let rom_selector: web_sys::HtmlInputElement =
                event.target().unwrap().dyn_into().unwrap();
            let file_list = rom_selector.files().unwrap();
            let file = file_list.get(0).unwrap();
            file_reader.read_as_array_buffer(&file);
            {
                let state = Rc::clone(&state);
                let disassembler_output_div = Rc::clone(&disassembler_output_div);
                let debug_output_div = Rc::clone(&debug_output_div);
                // Most of below based on this github issue: https://github.com/rustwasm/wasm-bindgen/issues/1292
                let mut closure = Closure::wrap(Box::new(move |event: web_sys::Event| {
                    let file_reader: web_sys::FileReader =
                        event.target().unwrap().dyn_into().unwrap();
                    let rom = file_reader.result().unwrap();
                    let rom = js_sys::Uint8Array::new(&rom);
                    let mut rom_vec: Vec<u8> = vec![0; rom.length() as usize];
                    rom.copy_to(&mut rom_vec);

                    state.borrow_mut().set_cpu(rom_vec);
                    let mut debug_string = String::new();
                    for byte in &state.borrow().rom.as_ref().unwrap().prg {
                        debug_string.push_str(&format!("{:X} ", byte));
                    }
                    console::log_1(&JsValue::from_str(&debug_string));
                    let disassembler_output = &state
                        .borrow()
                        .rom
                        .as_ref()
                        .unwrap()
                        .disassemble_prg_rom()
                        .unwrap();
                    // FIXME: Make document a Rc RefCell which will allow borrows correctly in this closure.
                    let document = web_sys::window().unwrap().document().unwrap();
                    let node = document.create_text_node(disassembler_output);
                    disassembler_output_div
                        .borrow_mut()
                        .append_child(&node)
                        .unwrap();

                    // FIXME: fix below
                    let mut nestest_output = String::new();
                    let mut last_pc = 0;
                    let mut loc_state = state.borrow_mut();
                    let cpu = loc_state.cpu.as_mut().unwrap();
                    while cpu.total_cycles <= 26554 {
                        let debug = cpu.debug_clock();
                        if last_pc != cpu.pc {
                            nestest_output.push_str(&format!("{}\n", debug).to_string());
                            last_pc = cpu.pc;
                        }
                    }

                    let debug_node = document.create_text_node(&nestest_output);
                    debug_output_div
                        .borrow_mut()
                        .append_child(&debug_node)
                        .unwrap();
                }) as Box<dyn FnMut(_)>);
                file_reader.set_onload(Some(closure.as_ref().unchecked_ref()));
                closure.forget();
            }
        }) as Box<dyn FnMut(_)>);
        rom_selector
            .add_event_listener_with_callback("change", closure.as_ref().unchecked_ref())?;
        closure.forget();
    }

    Ok(())
}
