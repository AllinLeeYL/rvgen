

OUTPUT_ARCH( "riscv" )
ENTRY(_start)

SECTIONS
{
  . = 0x80000000;
  .text : { *(.text) }
  . = STRESS_SECTION_BASE;
  .stress_core : { *(.stress_core) }
  . = DATA_SECTION_BASE;
  .data : { *(.data) }
  .bss : { *(.bss) }
  . = TOHOST_ADDR;
  .tohost : { *(.tohost) }
  _end = .;
}
