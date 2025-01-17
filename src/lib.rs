#![cfg_attr(not(test), no_std)]
#![doc = include_str!("../README.md")]

pub mod arch;
extern crate alloc;
use alloc::{
    string::{String, ToString},
    vec::Vec,
};
use log::info;
use memory_addr::{VirtAddr, PAGE_SIZE_4K};

use page_table_entry::MappingFlags;

mod auxv;
pub use auxv::auxv_vector;
pub use user_stack::app_stack_region;
mod user_stack;

/// Infomation about the elf segment, which is used to map the elf file to the memory space
pub struct ELFSegment {
    /// The start [`VirtAddr`] of the segment
    pub vaddr: VirtAddr,
    /// Size of the segment
    pub size: usize,
    /// [`MappingFlags`] of the segment which is used to set the page table entry
    pub flags: MappingFlags,
    /// The data of the segment
    pub data: Option<Vec<u8>>,
}

/// Base address of the ELF file loaded into the memory.
///
/// - When the ELF file is a **position-independent executable**,
/// the base address will be decided by the kernel.
///
/// - Otherwise, the base address **is determined by the file**, and this field `given_base` will be ignored.
///
/// # Arguments
///
/// * `elf` - The [`xmas_elf::ElfFile`] data
///
/// * `given_base` - The base address of the ELF file given by the kernel
///
/// # Return
///
/// The real base address for ELF file loaded into the memory.
pub fn elf_base_addr(elf: &xmas_elf::ElfFile, given_base: usize) -> Result<usize, String> {
    // Some elf will load ELF Header (offset == 0) to vaddr 0. In that case, base_addr will be added to all the LOAD.
    if elf.header.pt2.type_().as_type() == xmas_elf::header::Type::Executable {
        if let Some(ph) = elf
            .program_iter()
            .find(|ph| ph.get_type() == Ok(xmas_elf::program::Type::Load))
        {
            // The LOAD segements are sorted by the virtual address, so the first one is the lowest one.
            if ph.virtual_addr() == 0 {
                Err(
                    "The ELF file is an executable, but some segements may be loaded to vaddr 0"
                        .to_string(),
                )
            } else {
                Ok(0)
            }
        } else {
            Err("The ELF file is an executable, but no LOAD segment found".to_string())
        }
    } else {
        Ok(given_base)
    }
}

/// Read all [`self::ELFSegment`] with `LOAD` type of the elf file.
///
/// # Arguments
///
/// * `elf_data` - The elf file data
/// * `base_addr` - The base address of the elf file if the file will be loaded to the memory
///
/// # Warning
/// It can't be used to parse the elf file **which need the dynamic linker**, but you can do this **by calling this function recursively.**
pub fn elf_segments(elf: &xmas_elf::ElfFile, base_addr: usize) -> Vec<ELFSegment> {
    let elf_header = elf.header;
    let magic = elf_header.pt1.magic;
    assert_eq!(magic, [0x7f, 0x45, 0x4c, 0x46], "invalid elf!");

    info!("Base addr for the elf: 0x{:x}", base_addr);
    let mut segments = Vec::new();
    // Load Elf "LOAD" segments at base_addr.
    elf.program_iter()
        .filter(|ph| ph.get_type() == Ok(xmas_elf::program::Type::Load))
        .for_each(|ph| {
            let mut start_va = ph.virtual_addr() as usize + base_addr;
            let end_va = (ph.virtual_addr() + ph.mem_size()) as usize + base_addr;
            let mut start_offset = ph.offset() as usize;
            let end_offset = (ph.offset() + ph.file_size()) as usize;

            // Virtual address from elf may not be aligned.
            assert_eq!(start_va % PAGE_SIZE_4K, start_offset % PAGE_SIZE_4K);
            let front_pad = start_va % PAGE_SIZE_4K;
            start_va -= front_pad;
            start_offset -= front_pad;

            let mut flags = MappingFlags::USER;
            if ph.flags().is_read() {
                flags |= MappingFlags::READ;
            }
            if ph.flags().is_write() {
                flags |= MappingFlags::WRITE;
            }
            if ph.flags().is_execute() {
                flags |= MappingFlags::EXECUTE;
            }
            let data = Some(elf.input[start_offset..end_offset].to_vec());
            segments.push(ELFSegment {
                vaddr: VirtAddr::from(start_va),
                size: end_va - start_va,
                flags,
                data,
            });
        });

    segments
}

/// Read the entry point of the elf file
///
/// # Arguments
///
/// * `elf_data` - The elf file data
/// * `base_addr` - The base address of the elf file if the file will be loaded to the memory
///
/// # Warning
/// It can't be used to parse the elf file which need the dynamic linker, but you can do this by calling this function recursively
pub fn elf_entry(elf: &xmas_elf::ElfFile, base_addr: usize) -> VirtAddr {
    let elf_header = elf.header;
    let magic = elf_header.pt1.magic;
    assert_eq!(magic, [0x7f, 0x45, 0x4c, 0x46], "invalid elf!");

    info!("Base addr for the elf: 0x{:x}", base_addr);

    let entry = elf.header.pt2.entry_point() as usize + base_addr;
    entry.into()
}
