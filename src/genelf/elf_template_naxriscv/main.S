

#ifdef TEXT_LEN
  .set LEN, TEXT_LEN
#else
  .set LEN, 0
#endif

# Semaphore and barrier variables at fixed addresses, initialized to 0
.section .data
  .globl semaphore_1
  .globl barrier_count
  .globl barrier_phase
  .globl coresync_addrs
semaphore_1:
    .dword 0
barrier_count:
    .dword 0
barrier_phase:
    .dword 0
coresync_addrs:
    .zero 1024

.section .text
  .globl _start
  .align 2
_start:
  .rept (LEN - (. - _start)) / 4
  // use nop to pad for rvdump to work
  // use .word 0x0 for maybe more efficiency
  nop
  .endr

.section .stress_core,"ax",@progbits
  .globl _stress_code
  .align 2
_stress_code:
#ifdef STRESS_CORE_ENABLE
  .rept 0x1000 / 4
  nop
  .endr
#endif

.section .pass_section, "ax"
pass:
  j pass
