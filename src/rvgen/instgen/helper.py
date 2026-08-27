from enum import auto, IntEnum

class ISAInstrClass(IntEnum):
    REGFSM = auto()
    FPUFSM = auto()
    ALU = auto()
    ALU64 = auto()
    MULDIV = auto()
    MULDIV64 = auto()
    AMO = auto()
    AMO64 = auto()
    JAL = auto()
    JALR = auto()
    BRANCH = auto()
    MEM = auto()
    MEM64 = auto()
    MEMFPU = auto()
    FPU = auto()
    FPU64 = auto()
    MEMFPUD = auto()
    FPUD = auto()
    FPUD64 = auto()
    EXCEPTION = auto()
    RANDOM_CSR = auto()
    DESCEND_PRV = auto()
    FENCE = auto()
    WFI_TRAP = auto()
    WFI_NOTRAP = auto()
    SEND_IPI = auto()
    SEND_LOCAL_INTERRUPT = auto()
    CLEAR_INTERRUPT = auto()
    # CSR non-random
    EPCFSM = auto()
    TVECFSM = auto()
    MACHINE_CSR = auto()
    # TRIGGER_PENDING_INTERRUPT = auto()
    FREE_POLLUTED = auto()
    CREATE_ADDR_DEP = auto()
    # non-fuzzing, utilities
    CLEAR_MDT = auto()  # only for smdbltrp extention
    RESET_MCM = auto()
    WRITE_INSTR = auto()
    WAIT_FOR_INSTR = auto()

BASE_DISTRIBUTION = {
    ISAInstrClass.ALU: 0.1,
    ISAInstrClass.ALU64: 0.1,
    ISAInstrClass.MULDIV: 0.1,
    ISAInstrClass.MULDIV64: 0.1,
    ISAInstrClass.JAL: 0.01,
    ISAInstrClass.JALR: 0.01,
    ISAInstrClass.BRANCH: 0.4,
    ISAInstrClass.MEM: 0.5,
    ISAInstrClass.MEM64: 0.5,
    ISAInstrClass.AMO: 0.0, # not supported, stores must be larger than loads
    ISAInstrClass.AMO64: 0.0,
    ISAInstrClass.FENCE: 0.000001,
    ISAInstrClass.MEMFPU: 0,
    ISAInstrClass.MEMFPUD: 0,
    ISAInstrClass.FPUFSM: 0,
    ISAInstrClass.FPU: 0,
    ISAInstrClass.FPU64: 0,
    ISAInstrClass.FPUD: 0,
    ISAInstrClass.FPUD64: 0,
    ISAInstrClass.REGFSM: 0.1,
    ISAInstrClass.TVECFSM: 0.1,
    ISAInstrClass.EPCFSM: 0.1,
    ISAInstrClass.FREE_POLLUTED: 5.0,       # utility for MCM fuzzing
    ISAInstrClass.CREATE_ADDR_DEP: 2.0,  # utility for MCM fuzzing
    ISAInstrClass.MACHINE_CSR: 0.1,
    ISAInstrClass.EXCEPTION: 0.1,
    ISAInstrClass.RANDOM_CSR: 0.0,
    ISAInstrClass.DESCEND_PRV: 0.1,
    ISAInstrClass.WFI_TRAP: 0.7,
    ISAInstrClass.WFI_NOTRAP: 0.4,
    ISAInstrClass.SEND_IPI: 5.0,
    ISAInstrClass.SEND_LOCAL_INTERRUPT: 0,
    ISAInstrClass.CLEAR_INTERRUPT: 0.4,
    ISAInstrClass.WRITE_INSTR: 0,
    ISAInstrClass.WAIT_FOR_INSTR: 0.0,
    ISAInstrClass.CLEAR_MDT: 0.0,
    ISAInstrClass.RESET_MCM: 0.0
}
