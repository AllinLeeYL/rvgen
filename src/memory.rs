//! Physical RAM constraints and the sections allocated by generation.
use anyhow::{Result, anyhow, ensure};

use crate::riscv::Xlen;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Permissions {
    pub write: bool,
    pub execute: bool,
}

impl Default for Permissions {
    fn default() -> Self {
        Self {
            write: true,
            execute: true,
        }
    }
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
}

#[derive(Debug, Clone, Default)]
pub struct MemoryRegion {
    pub start: u64,
    pub size: u64,
    pub permissions: Permissions,
}

impl MemoryRegion {
    pub fn end(&self) -> Result<u64> {
        ensure!(self.size > 0, "memory region must not be empty");
        self.start
            .checked_add(self.size)
            .ok_or_else(|| anyhow!("memory region address overflow"))
    }
}

#[derive(Debug, Clone, Default)]
pub struct Section {
    pub name: String,
    pub alignment: u64,
    pub permissions: Permissions,
    pub region: MemoryRegion,
    /// Owned by the runtime (e.g. the HTIF mailboxes): generated data accesses
    /// never target it.
    pub private: bool,
}

#[derive(Debug, Clone, Default)]
pub struct MemoryLayout {
    pub sections: Vec<Section>,
}

impl MemoryLayout {
    pub fn add(&mut self, section: Section) {
        self.sections.push(section);
    }

    pub fn get(&self, name: &str) -> Result<&Section> {
        self.sections
            .iter()
            .find(|section| section.name == name)
            .ok_or_else(|| anyhow!("missing section {name}"))
    }

    /// Writable sections available to generated data accesses. RW sections are
    /// preferred; RWX sections are only offered when no RW section exists.
    pub fn data_sections(&self) -> impl Iterator<Item = &Section> {
        let writable = || {
            self.sections
                .iter()
                .filter(|section| section.permissions.write && !section.private)
        };
        let has_rw = writable().any(|section| !section.permissions.execute);
        writable().filter(move |section| !(has_rw && section.permissions.execute))
    }

    /// Reserve data from the top of RAM, leaving the base available for code.
    pub fn reserve(
        &mut self,
        ram: &MemoryRegion,
        name: &str,
        size: u64,
        alignment: u64,
        permissions: Permissions,
    ) -> Result<&mut Section> {
        ensure!(size > 0, "section {name} must not be empty");
        ensure!(alignment.is_power_of_two(), "invalid section alignment");
        let end = self
            .sections
            .iter()
            .map(|section| section.region.start)
            .min()
            .unwrap_or(ram.end()?);
        let start = end
            .checked_sub(size)
            .ok_or_else(|| anyhow!("section {name} does not fit RAM"))?
            & !(alignment - 1);
        ensure!(start >= ram.start, "section {name} does not fit RAM");
        ensure!(
            ram.permissions.permits(permissions),
            "section {name} permissions exceed RAM permissions"
        );
        self.add(Section {
            name: name.into(),
            alignment,
            permissions,
            region: MemoryRegion {
                start,
                size,
                permissions,
            },
            private: false,
        });
        Ok(self.sections.last_mut().expect("section was just added"))
    }

    pub fn validate(&self, ram: &MemoryRegion, xlen: Xlen) -> Result<()> {
        let ram_end = ram.end()?;
        if xlen == Xlen::X32 {
            ensure!(ram_end <= 1u64 << 32, "RAM exceeds the RV32 address space");
        }
        let mut names = std::collections::HashSet::new();
        for (index, section) in self.sections.iter().enumerate() {
            ensure!(
                names.insert(&section.name),
                "duplicate section {}",
                section.name
            );
            ensure!(
                section.alignment.is_power_of_two(),
                "invalid section alignment"
            );
            ensure!(
                section.region.start % section.alignment == 0,
                "misaligned section {}",
                section.name
            );
            let end = section.region.end()?;
            ensure!(
                section.region.start >= ram.start && end <= ram_end,
                "section {} does not fit RAM",
                section.name
            );
            ensure!(
                ram.permissions.permits(section.permissions),
                "section {} permissions exceed RAM permissions",
                section.name
            );
            for other in &self.sections[..index] {
                ensure!(
                    end <= other.region.start || section.region.start >= other.region.end()?,
                    "sections {} and {} overlap",
                    section.name,
                    other.name
                );
            }
        }
        Ok(())
    }
}

