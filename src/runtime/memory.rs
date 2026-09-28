//! Physical RAM constraints, requested sections, and one resolved allocation map.
//! ELF allocation describes the image; it does not configure RAM, PMP, or paging.
use anyhow::{Result, anyhow, bail, ensure};

use crate::riscv::Xlen;
use crate::utils::{ElfSection, MAX_ELF_SIZE, SHF_ALLOC, SHF_EXECINSTR, SHF_WRITE};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Permissions {
    pub write: bool,
    pub execute: bool,
}

impl Permissions {
    // All supported image sections are readable.
    pub const RW: Self = Self {
        write: true,
        execute: false,
    };
    pub const RX: Self = Self {
        write: false,
        execute: true,
    };
    pub const RWX: Self = Self {
        write: true,
        execute: true,
    };

    fn permits(self, required: Self) -> bool {
        (!required.write || self.write) && (!required.execute || self.execute)
    }

    fn elf_flags(self) -> u64 {
        SHF_ALLOC
            | if self.write { SHF_WRITE } else { 0 }
            | if self.execute { SHF_EXECINSTR } else { 0 }
    }
}

#[derive(Debug, Clone)]
pub struct MemoryRegion {
    pub start: u64,
    pub size: u64,
    pub permissions: Permissions,
}

impl MemoryRegion {
    fn end(&self) -> Result<u64> {
        ensure!(self.size > 0, "memory region must not be empty");
        self.start
            .checked_add(self.size)
            .ok_or_else(|| anyhow!("memory region address overflow"))
    }
}

/// Add another request to reserve a new section; names identify it throughout
/// layout, address fixups, symbols, and ELF emission (no fixed section indices).
#[derive(Debug, Clone)]
pub struct SectionRequest {
    pub name: String,
    pub size: usize,
    pub alignment: u64,
    pub permissions: Permissions,
}

impl SectionRequest {
    pub fn new(name: &str, size: usize, alignment: u64, permissions: Permissions) -> Self {
        Self {
            name: name.into(),
            size,
            alignment,
            permissions,
        }
    }
}

#[derive(Debug, Clone)]
pub struct MemoryPlan {
    regions: Vec<MemoryRegion>,
    entry: u64,
    requests: Vec<SectionRequest>,
}

impl MemoryPlan {
    pub fn new(
        mut regions: Vec<MemoryRegion>,
        entry: u64,
        requests: Vec<SectionRequest>,
    ) -> Result<Self> {
        ensure!(!regions.is_empty(), "at least one RAM region is required");
        regions.sort_by_key(|region| region.start);
        for (index, region) in regions.iter().enumerate() {
            region.end()?;
            if index > 0 {
                ensure!(
                    regions[index - 1].end()? <= region.start,
                    "RAM regions overlap"
                );
            }
        }
        ensure!(entry % 4 == 0, "runtime entry must be four-byte aligned");
        let mut names =
            std::collections::HashSet::from([".text", ".symtab", ".strtab", ".shstrtab"]);
        for request in &requests {
            ensure!(
                !request.name.is_empty() && request.name.bytes().all(|b| b != 0 && b < 128),
                "invalid section name"
            );
            ensure!(
                names.insert(&request.name),
                "duplicate or reserved section name: {}",
                request.name
            );
            ensure!(
                request.size > 0,
                "section {} must not be empty",
                request.name
            );
            ensure!(
                request.alignment.is_power_of_two(),
                "section {} alignment must be a power of two",
                request.name
            );
        }
        Ok(Self {
            regions,
            entry,
            requests,
        })
    }

    pub fn bare_metal(base: u64, size: u64, scratch_size: usize, smc_size: usize) -> Result<Self> {
        let mut requests = vec![
            SectionRequest::new(".tohost", 8, 64, Permissions::RW),
            SectionRequest::new(".fromhost", 8, 64, Permissions::RW),
            SectionRequest::new(".scratch", scratch_size, 64, Permissions::RW),
        ];
        if smc_size > 0 {
            requests.push(SectionRequest::new(".smc", smc_size, 64, Permissions::RWX));
        }
        Self::new(
            vec![MemoryRegion {
                start: base,
                size,
                permissions: Permissions::RWX,
            }],
            base,
            requests,
        )
    }

    pub fn entry(&self) -> u64 {
        self.entry
    }

    pub fn request(&self, name: &str) -> Option<&SectionRequest> {
        self.requests.iter().find(|request| request.name == name)
    }

    pub fn validate_xlen(&self, xlen: Xlen) -> Result<()> {
        if xlen == Xlen::X32 {
            for region in &self.regions {
                ensure!(
                    region.end()? <= 1u64 << 32,
                    "RAM region exceeds the RV32 address space"
                );
            }
        }
        Ok(())
    }

    pub fn resolve(&self, text_size: usize) -> Result<MemoryLayout> {
        let total = self
            .requests
            .iter()
            .try_fold(text_size as u64, |total, request| {
                total
                    .checked_add(request.size as u64)
                    .ok_or_else(|| anyhow!("image size overflow"))
            })?;
        ensure!(
            total <= MAX_ELF_SIZE,
            "requested sections exceed the ELF size limit"
        );
        let text = SectionRequest::new(".text", text_size, 4, Permissions::RX);
        let text_end = self
            .entry
            .checked_add(text_size as u64)
            .ok_or_else(|| anyhow!("text address overflow"))?;
        ensure!(
            self.regions.iter().any(|r| r.start <= self.entry
                && r.end().is_ok_and(|end| text_end <= end)
                && r.permissions.permits(text.permissions)),
            ".text does not fit executable RAM at {:#x}",
            self.entry
        );
        let mut allocations = vec![Allocation {
            request: text,
            address: self.entry,
        }];
        for request in &self.requests {
            let mut selected = None;
            for region in &self.regions {
                if !region.permissions.permits(request.permissions) {
                    continue;
                }
                let mut candidate = align_up(region.start, request.alignment)?;
                loop {
                    let end = candidate
                        .checked_add(request.size as u64)
                        .ok_or_else(|| anyhow!("section address overflow"))?;
                    if end > region.end()? {
                        break;
                    }
                    if let Some(overlap) = allocations
                        .iter()
                        .find(|a| candidate < a.end() && a.address < end)
                    {
                        candidate = align_up(overlap.end(), request.alignment)?;
                    } else {
                        selected = Some(candidate);
                        break;
                    }
                }
                if selected.is_some() {
                    break;
                }
            }
            let Some(address) = selected else {
                bail!(
                    "section {} does not fit RAM with the required permissions",
                    request.name
                );
            };
            allocations.push(Allocation {
                request: request.clone(),
                address,
            });
        }
        Ok(MemoryLayout { allocations })
    }
}

fn align_up(address: u64, alignment: u64) -> Result<u64> {
    address
        .checked_add(alignment - 1)
        .map(|v| v & !(alignment - 1))
        .ok_or_else(|| anyhow!("section alignment overflow"))
}

#[derive(Debug)]
pub struct Allocation {
    pub request: SectionRequest,
    pub address: u64,
}

impl Allocation {
    fn end(&self) -> u64 {
        self.address + self.request.size as u64
    }
}

#[derive(Debug)]
pub struct MemoryLayout {
    allocations: Vec<Allocation>,
}

impl MemoryLayout {
    pub fn index(&self, name: &str) -> Result<usize> {
        self.allocations
            .iter()
            .position(|allocation| allocation.request.name == name)
            .ok_or_else(|| anyhow!("missing section {name}"))
    }

    pub fn get(&self, name: &str) -> Result<&Allocation> {
        Ok(&self.allocations[self.index(name)?])
    }

    pub fn sections(&self, text: Vec<u8>) -> Result<Vec<ElfSection>> {
        ensure!(
            text.len() == self.get(".text")?.request.size,
            "text size changed after memory layout"
        );
        let mut text = Some(text);
        Ok(self
            .allocations
            .iter()
            .map(|allocation| {
                let request = &allocation.request;
                let bytes = if request.name == ".text" {
                    text.take().unwrap()
                } else {
                    vec![0; request.size]
                };
                let mut section = ElfSection::new(&request.name, bytes);
                section.addr = allocation.address;
                section.align = request.alignment;
                section.flags = request.permissions.elf_flags();
                section
            })
            .collect())
    }
}
