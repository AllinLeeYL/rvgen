// Frozen outputs from the original Python functions; no Python is needed to run these tests.
#[test]
fn all_encoders_match_python() {
    let mut count = 0;
    for row in include_str!("fixtures/encodings.tsv").lines() {
        let fields: Vec<_> = row.split('\t').collect();
        let a: Vec<i32> = fields[1]
            .split(',')
            .filter(|s| !s.is_empty())
            .map(|s| s.parse().unwrap())
            .collect();
        let actual = match fields[0] {
            "instruc_rtype" => {
                rvgen::riscv::rvprotoinstrs::instruc_rtype(a[0], a[1], a[2], a[3], a[4], a[5])
            }
            "instruc_itype" => {
                rvgen::riscv::rvprotoinstrs::instruc_itype(a[0], a[1], a[2], a[3], a[4])
            }
            "instruc_stype" => {
                rvgen::riscv::rvprotoinstrs::instruc_stype(a[0], a[1], a[2], a[3], a[4])
            }
            "instruc_btype" => {
                rvgen::riscv::rvprotoinstrs::instruc_btype(a[0], a[1], a[2], a[3], a[4])
            }
            "instruc_utype" => rvgen::riscv::rvprotoinstrs::instruc_utype(a[0], a[1], a[2]),
            "instruc_jtype" => rvgen::riscv::rvprotoinstrs::instruc_jtype(a[0], a[1], a[2]),
            "instruc_r4type" => rvgen::riscv::rvprotoinstrs::instruc_r4type(
                a[0], a[1], a[2], a[3], a[4], a[5], a[6],
            ),
            "instruc_crtype" => rvgen::riscv::rvprotoinstrs::instruc_crtype(a[0], a[1], a[2], a[3]),
            "instruc_citype" => rvgen::riscv::rvprotoinstrs::instruc_citype(a[0], a[1], a[2], a[3]),
            "instruc_csstype" => {
                rvgen::riscv::rvprotoinstrs::instruc_csstype(a[0], a[1], a[2], a[3])
            }
            "instruc_ciwtype" => {
                rvgen::riscv::rvprotoinstrs::instruc_ciwtype(a[0], a[1], a[2], a[3])
            }
            "instruc_cltype" => {
                rvgen::riscv::rvprotoinstrs::instruc_cltype(a[0], a[1], a[2], a[3], a[4])
            }
            "instruc_cstype" => {
                rvgen::riscv::rvprotoinstrs::instruc_cstype(a[0], a[1], a[2], a[3], a[4])
            }
            "instruc_catype" => {
                rvgen::riscv::rvprotoinstrs::instruc_catype(a[0], a[1], a[2], a[3], a[4])
            }
            "instruc_cbtype" => rvgen::riscv::rvprotoinstrs::instruc_cbtype(a[0], a[1], a[2], a[3]),
            "instruc_cjtype" => rvgen::riscv::rvprotoinstrs::instruc_cjtype(a[0], a[1], a[2]),
            "rv32i_lui" => rvgen::riscv::rv32i::rv32i_lui(a[0], a[1]),
            "rv32i_auipc" => rvgen::riscv::rv32i::rv32i_auipc(a[0], a[1]),
            "rv32i_jal" => rvgen::riscv::rv32i::rv32i_jal(a[0], a[1]),
            "rv32i_jalr" => rvgen::riscv::rv32i::rv32i_jalr(a[0], a[1], a[2]),
            "rv32i_beq" => rvgen::riscv::rv32i::rv32i_beq(a[0], a[1], a[2]),
            "rv32i_bne" => rvgen::riscv::rv32i::rv32i_bne(a[0], a[1], a[2]),
            "rv32i_blt" => rvgen::riscv::rv32i::rv32i_blt(a[0], a[1], a[2]),
            "rv32i_bge" => rvgen::riscv::rv32i::rv32i_bge(a[0], a[1], a[2]),
            "rv32i_bltu" => rvgen::riscv::rv32i::rv32i_bltu(a[0], a[1], a[2]),
            "rv32i_bgeu" => rvgen::riscv::rv32i::rv32i_bgeu(a[0], a[1], a[2]),
            "rv32i_lb" => rvgen::riscv::rv32i::rv32i_lb(a[0], a[1], a[2]),
            "rv32i_lh" => rvgen::riscv::rv32i::rv32i_lh(a[0], a[1], a[2]),
            "rv32i_lw" => rvgen::riscv::rv32i::rv32i_lw(a[0], a[1], a[2]),
            "rv32i_lbu" => rvgen::riscv::rv32i::rv32i_lbu(a[0], a[1], a[2]),
            "rv32i_lhu" => rvgen::riscv::rv32i::rv32i_lhu(a[0], a[1], a[2]),
            "rv32i_sb" => rvgen::riscv::rv32i::rv32i_sb(a[0], a[1], a[2]),
            "rv32i_sh" => rvgen::riscv::rv32i::rv32i_sh(a[0], a[1], a[2]),
            "rv32i_sw" => rvgen::riscv::rv32i::rv32i_sw(a[0], a[1], a[2]),
            "rv32i_addi" => rvgen::riscv::rv32i::rv32i_addi(a[0], a[1], a[2]),
            "rv32i_slti" => rvgen::riscv::rv32i::rv32i_slti(a[0], a[1], a[2]),
            "rv32i_sltiu" => rvgen::riscv::rv32i::rv32i_sltiu(a[0], a[1], a[2]),
            "rv32i_xori" => rvgen::riscv::rv32i::rv32i_xori(a[0], a[1], a[2]),
            "rv32i_ori" => rvgen::riscv::rv32i::rv32i_ori(a[0], a[1], a[2]),
            "rv32i_andi" => rvgen::riscv::rv32i::rv32i_andi(a[0], a[1], a[2]),
            "rv32i_slli" => rvgen::riscv::rv32i::rv32i_slli(a[0], a[1], a[2]),
            "rv32i_srli" => rvgen::riscv::rv32i::rv32i_srli(a[0], a[1], a[2]),
            "rv32i_srai" => rvgen::riscv::rv32i::rv32i_srai(a[0], a[1], a[2]),
            "rv32i_add" => rvgen::riscv::rv32i::rv32i_add(a[0], a[1], a[2]),
            "rv32i_sub" => rvgen::riscv::rv32i::rv32i_sub(a[0], a[1], a[2]),
            "rv32i_sll" => rvgen::riscv::rv32i::rv32i_sll(a[0], a[1], a[2]),
            "rv32i_slt" => rvgen::riscv::rv32i::rv32i_slt(a[0], a[1], a[2]),
            "rv32i_sltu" => rvgen::riscv::rv32i::rv32i_sltu(a[0], a[1], a[2]),
            "rv32i_xor" => rvgen::riscv::rv32i::rv32i_xor(a[0], a[1], a[2]),
            "rv32i_srl" => rvgen::riscv::rv32i::rv32i_srl(a[0], a[1], a[2]),
            "rv32i_sra" => rvgen::riscv::rv32i::rv32i_sra(a[0], a[1], a[2]),
            "rv32i_or" => rvgen::riscv::rv32i::rv32i_or(a[0], a[1], a[2]),
            "rv32i_and" => rvgen::riscv::rv32i::rv32i_and(a[0], a[1], a[2]),
            "rv32i_fence" => rvgen::riscv::rv32i::rv32i_fence(a[0]),
            "rv32i_ecall" => rvgen::riscv::rv32i::rv32i_ecall(),
            "rv32i_ebreak" => rvgen::riscv::rv32i::rv32i_ebreak(),
            "rv64i_lwu" => rvgen::riscv::rv64i::rv64i_lwu(a[0], a[1], a[2]),
            "rv64i_ld" => rvgen::riscv::rv64i::rv64i_ld(a[0], a[1], a[2]),
            "rv64i_sd" => rvgen::riscv::rv64i::rv64i_sd(a[0], a[1], a[2]),
            "rv64i_addiw" => rvgen::riscv::rv64i::rv64i_addiw(a[0], a[1], a[2]),
            "rv64i_slliw" => rvgen::riscv::rv64i::rv64i_slliw(a[0], a[1], a[2]),
            "rv64i_srliw" => rvgen::riscv::rv64i::rv64i_srliw(a[0], a[1], a[2]),
            "rv64i_sraiw" => rvgen::riscv::rv64i::rv64i_sraiw(a[0], a[1], a[2]),
            "rv64i_addw" => rvgen::riscv::rv64i::rv64i_addw(a[0], a[1], a[2]),
            "rv64i_subw" => rvgen::riscv::rv64i::rv64i_subw(a[0], a[1], a[2]),
            "rv64i_sllw" => rvgen::riscv::rv64i::rv64i_sllw(a[0], a[1], a[2]),
            "rv64i_srlw" => rvgen::riscv::rv64i::rv64i_srlw(a[0], a[1], a[2]),
            "rv64i_sraw" => rvgen::riscv::rv64i::rv64i_sraw(a[0], a[1], a[2]),
            "rv32m_mul" => rvgen::riscv::rv32m::rv32m_mul(a[0], a[1], a[2]),
            "rv32m_mulh" => rvgen::riscv::rv32m::rv32m_mulh(a[0], a[1], a[2]),
            "rv32m_mulhsu" => rvgen::riscv::rv32m::rv32m_mulhsu(a[0], a[1], a[2]),
            "rv32m_mulhu" => rvgen::riscv::rv32m::rv32m_mulhu(a[0], a[1], a[2]),
            "rv32m_div" => rvgen::riscv::rv32m::rv32m_div(a[0], a[1], a[2]),
            "rv32m_divu" => rvgen::riscv::rv32m::rv32m_divu(a[0], a[1], a[2]),
            "rv32m_rem" => rvgen::riscv::rv32m::rv32m_rem(a[0], a[1], a[2]),
            "rv32m_remu" => rvgen::riscv::rv32m::rv32m_remu(a[0], a[1], a[2]),
            "rv64m_mulw" => rvgen::riscv::rv64m::rv64m_mulw(a[0], a[1], a[2]),
            "rv64m_divw" => rvgen::riscv::rv64m::rv64m_divw(a[0], a[1], a[2]),
            "rv64m_divuw" => rvgen::riscv::rv64m::rv64m_divuw(a[0], a[1], a[2]),
            "rv64m_remw" => rvgen::riscv::rv64m::rv64m_remw(a[0], a[1], a[2]),
            "rv64m_remuw" => rvgen::riscv::rv64m::rv64m_remuw(a[0], a[1], a[2]),
            "rv32a_lrw" => rvgen::riscv::rv32a::rv32a_lrw(a[0] != 0, a[1] != 0, a[2], a[3], a[4]),
            "rv32a_scw" => rvgen::riscv::rv32a::rv32a_scw(a[0] != 0, a[1] != 0, a[2], a[3], a[4]),
            "rv32a_amoswapw" => {
                rvgen::riscv::rv32a::rv32a_amoswapw(a[0] != 0, a[1] != 0, a[2], a[3], a[4])
            }
            "rv32a_amoaddw" => {
                rvgen::riscv::rv32a::rv32a_amoaddw(a[0] != 0, a[1] != 0, a[2], a[3], a[4])
            }
            "rv32a_amoandw" => {
                rvgen::riscv::rv32a::rv32a_amoandw(a[0] != 0, a[1] != 0, a[2], a[3], a[4])
            }
            "rv32a_amoorw" => {
                rvgen::riscv::rv32a::rv32a_amoorw(a[0] != 0, a[1] != 0, a[2], a[3], a[4])
            }
            "rv32a_amoxorw" => {
                rvgen::riscv::rv32a::rv32a_amoxorw(a[0] != 0, a[1] != 0, a[2], a[3], a[4])
            }
            "rv32a_amomaxw" => {
                rvgen::riscv::rv32a::rv32a_amomaxw(a[0] != 0, a[1] != 0, a[2], a[3], a[4])
            }
            "rv32a_amomaxuw" => {
                rvgen::riscv::rv32a::rv32a_amomaxuw(a[0] != 0, a[1] != 0, a[2], a[3], a[4])
            }
            "rv32a_amominw" => {
                rvgen::riscv::rv32a::rv32a_amominw(a[0] != 0, a[1] != 0, a[2], a[3], a[4])
            }
            "rv32a_amominuw" => {
                rvgen::riscv::rv32a::rv32a_amominuw(a[0] != 0, a[1] != 0, a[2], a[3], a[4])
            }
            "rv64a_lrd" => rvgen::riscv::rv64a::rv64a_lrd(a[0] != 0, a[1] != 0, a[2], a[3], a[4]),
            "rv64a_scd" => rvgen::riscv::rv64a::rv64a_scd(a[0] != 0, a[1] != 0, a[2], a[3], a[4]),
            "rv64a_amoswapd" => {
                rvgen::riscv::rv64a::rv64a_amoswapd(a[0] != 0, a[1] != 0, a[2], a[3], a[4])
            }
            "rv64a_amoaddd" => {
                rvgen::riscv::rv64a::rv64a_amoaddd(a[0] != 0, a[1] != 0, a[2], a[3], a[4])
            }
            "rv64a_amoandd" => {
                rvgen::riscv::rv64a::rv64a_amoandd(a[0] != 0, a[1] != 0, a[2], a[3], a[4])
            }
            "rv64a_amoord" => {
                rvgen::riscv::rv64a::rv64a_amoord(a[0] != 0, a[1] != 0, a[2], a[3], a[4])
            }
            "rv64a_amoxord" => {
                rvgen::riscv::rv64a::rv64a_amoxord(a[0] != 0, a[1] != 0, a[2], a[3], a[4])
            }
            "rv64a_amomaxd" => {
                rvgen::riscv::rv64a::rv64a_amomaxd(a[0] != 0, a[1] != 0, a[2], a[3], a[4])
            }
            "rv64a_amomaxud" => {
                rvgen::riscv::rv64a::rv64a_amomaxud(a[0] != 0, a[1] != 0, a[2], a[3], a[4])
            }
            "rv64a_amomind" => {
                rvgen::riscv::rv64a::rv64a_amomind(a[0] != 0, a[1] != 0, a[2], a[3], a[4])
            }
            "rv64a_amominud" => {
                rvgen::riscv::rv64a::rv64a_amominud(a[0] != 0, a[1] != 0, a[2], a[3], a[4])
            }
            "rv32f_flw" => rvgen::riscv::rv32f::rv32f_flw(a[0], a[1], a[2]),
            "rv32f_fsw" => rvgen::riscv::rv32f::rv32f_fsw(a[0], a[1], a[2]),
            "rv32f_fmadds" => rvgen::riscv::rv32f::rv32f_fmadds(a[0], a[1], a[2], a[3], a[4]),
            "rv32f_fmsubs" => rvgen::riscv::rv32f::rv32f_fmsubs(a[0], a[1], a[2], a[3], a[4]),
            "rv32f_fnmsubs" => rvgen::riscv::rv32f::rv32f_fnmsubs(a[0], a[1], a[2], a[3], a[4]),
            "rv32f_fnmadds" => rvgen::riscv::rv32f::rv32f_fnmadds(a[0], a[1], a[2], a[3], a[4]),
            "rv32f_fadds" => rvgen::riscv::rv32f::rv32f_fadds(a[0], a[1], a[2], a[3]),
            "rv32f_fsubs" => rvgen::riscv::rv32f::rv32f_fsubs(a[0], a[1], a[2], a[3]),
            "rv32f_fmuls" => rvgen::riscv::rv32f::rv32f_fmuls(a[0], a[1], a[2], a[3]),
            "rv32f_fdivs" => rvgen::riscv::rv32f::rv32f_fdivs(a[0], a[1], a[2], a[3]),
            "rv32f_fsqrts" => rvgen::riscv::rv32f::rv32f_fsqrts(a[0], a[1], a[2]),
            "rv32f_fsgnjs" => rvgen::riscv::rv32f::rv32f_fsgnjs(a[0], a[1], a[2]),
            "rv32f_fsgnjns" => rvgen::riscv::rv32f::rv32f_fsgnjns(a[0], a[1], a[2]),
            "rv32f_fsgnjxs" => rvgen::riscv::rv32f::rv32f_fsgnjxs(a[0], a[1], a[2]),
            "rv32f_fmins" => rvgen::riscv::rv32f::rv32f_fmins(a[0], a[1], a[2]),
            "rv32f_fmaxs" => rvgen::riscv::rv32f::rv32f_fmaxs(a[0], a[1], a[2]),
            "rv32f_fcvtws" => rvgen::riscv::rv32f::rv32f_fcvtws(a[0], a[1], a[2]),
            "rv32f_fcvtwus" => rvgen::riscv::rv32f::rv32f_fcvtwus(a[0], a[1], a[2]),
            "rv32f_fmvxw" => rvgen::riscv::rv32f::rv32f_fmvxw(a[0], a[1]),
            "rv32f_feqs" => rvgen::riscv::rv32f::rv32f_feqs(a[0], a[1], a[2]),
            "rv32f_flts" => rvgen::riscv::rv32f::rv32f_flts(a[0], a[1], a[2]),
            "rv32f_fles" => rvgen::riscv::rv32f::rv32f_fles(a[0], a[1], a[2]),
            "rv32f_fclasss" => rvgen::riscv::rv32f::rv32f_fclasss(a[0], a[1]),
            "rv32f_fcvtsw" => rvgen::riscv::rv32f::rv32f_fcvtsw(a[0], a[1], a[2]),
            "rv32f_fcvtswu" => rvgen::riscv::rv32f::rv32f_fcvtswu(a[0], a[1], a[2]),
            "rv32f_fmvwx" => rvgen::riscv::rv32f::rv32f_fmvwx(a[0], a[1]),
            "rv64f_fcvtls" => rvgen::riscv::rv64f::rv64f_fcvtls(a[0], a[1], a[2]),
            "rv64f_fcvtlus" => rvgen::riscv::rv64f::rv64f_fcvtlus(a[0], a[1], a[2]),
            "rv64f_fcvtsl" => rvgen::riscv::rv64f::rv64f_fcvtsl(a[0], a[1], a[2]),
            "rv64f_fcvtslu" => rvgen::riscv::rv64f::rv64f_fcvtslu(a[0], a[1], a[2]),
            "rv32d_fld" => rvgen::riscv::rv32d::rv32d_fld(a[0], a[1], a[2]),
            "rv32d_fsd" => rvgen::riscv::rv32d::rv32d_fsd(a[0], a[1], a[2]),
            "rv32d_fmaddd" => rvgen::riscv::rv32d::rv32d_fmaddd(a[0], a[1], a[2], a[3], a[4]),
            "rv32d_fmsubd" => rvgen::riscv::rv32d::rv32d_fmsubd(a[0], a[1], a[2], a[3], a[4]),
            "rv32d_fnmsubd" => rvgen::riscv::rv32d::rv32d_fnmsubd(a[0], a[1], a[2], a[3], a[4]),
            "rv32d_fnmaddd" => rvgen::riscv::rv32d::rv32d_fnmaddd(a[0], a[1], a[2], a[3], a[4]),
            "rv32d_faddd" => rvgen::riscv::rv32d::rv32d_faddd(a[0], a[1], a[2], a[3]),
            "rv32d_fsubd" => rvgen::riscv::rv32d::rv32d_fsubd(a[0], a[1], a[2], a[3]),
            "rv32d_fmuld" => rvgen::riscv::rv32d::rv32d_fmuld(a[0], a[1], a[2], a[3]),
            "rv32d_fdivd" => rvgen::riscv::rv32d::rv32d_fdivd(a[0], a[1], a[2], a[3]),
            "rv32d_fsqrtd" => rvgen::riscv::rv32d::rv32d_fsqrtd(a[0], a[1], a[2]),
            "rv32d_fsgnjd" => rvgen::riscv::rv32d::rv32d_fsgnjd(a[0], a[1], a[2]),
            "rv32d_fsgnjnd" => rvgen::riscv::rv32d::rv32d_fsgnjnd(a[0], a[1], a[2]),
            "rv32d_fsgnjxd" => rvgen::riscv::rv32d::rv32d_fsgnjxd(a[0], a[1], a[2]),
            "rv32d_fmind" => rvgen::riscv::rv32d::rv32d_fmind(a[0], a[1], a[2]),
            "rv32d_fmaxd" => rvgen::riscv::rv32d::rv32d_fmaxd(a[0], a[1], a[2]),
            "rv32d_fcvtsd" => rvgen::riscv::rv32d::rv32d_fcvtsd(a[0], a[1], a[2]),
            "rv32d_fcvtds" => rvgen::riscv::rv32d::rv32d_fcvtds(a[0], a[1], a[2]),
            "rv32d_feqd" => rvgen::riscv::rv32d::rv32d_feqd(a[0], a[1], a[2]),
            "rv32d_fltd" => rvgen::riscv::rv32d::rv32d_fltd(a[0], a[1], a[2]),
            "rv32d_fled" => rvgen::riscv::rv32d::rv32d_fled(a[0], a[1], a[2]),
            "rv32d_fclassd" => rvgen::riscv::rv32d::rv32d_fclassd(a[0], a[1]),
            "rv32d_fcvtwd" => rvgen::riscv::rv32d::rv32d_fcvtwd(a[0], a[1], a[2]),
            "rv32d_fcvtwud" => rvgen::riscv::rv32d::rv32d_fcvtwud(a[0], a[1], a[2]),
            "rv32d_fcvtdw" => rvgen::riscv::rv32d::rv32d_fcvtdw(a[0], a[1], a[2]),
            "rv32d_fcvtdwu" => rvgen::riscv::rv32d::rv32d_fcvtdwu(a[0], a[1], a[2]),
            "rv64d_fcvtld" => rvgen::riscv::rv64d::rv64d_fcvtld(a[0], a[1], a[2]),
            "rv64d_fcvtlud" => rvgen::riscv::rv64d::rv64d_fcvtlud(a[0], a[1], a[2]),
            "rv64d_fmvxd" => rvgen::riscv::rv64d::rv64d_fmvxd(a[0], a[1]),
            "rv64d_fcvtdl" => rvgen::riscv::rv64d::rv64d_fcvtdl(a[0], a[1], a[2]),
            "rv64d_fcvtdlu" => rvgen::riscv::rv64d::rv64d_fcvtdlu(a[0], a[1], a[2]),
            "rv64d_fmvdx" => rvgen::riscv::rv64d::rv64d_fmvdx(a[0], a[1]),
            "rv32ic_mv" => rvgen::riscv::rv32c::rv32ic_mv(a[0], a[1]),
            "rv32ic_add" => rvgen::riscv::rv32c::rv32ic_add(a[0], a[1]),
            "rv32ic_and" => rvgen::riscv::rv32c::rv32ic_and(a[0], a[1]),
            "rv32ic_or" => rvgen::riscv::rv32c::rv32ic_or(a[0], a[1]),
            "rv32ic_xor" => rvgen::riscv::rv32c::rv32ic_xor(a[0], a[1]),
            "rv32ic_sub" => rvgen::riscv::rv32c::rv32ic_sub(a[0], a[1]),
            "rv32ic_lui" => rvgen::riscv::rv32c::rv32ic_lui(a[0], a[1]),
            "rv32ic_addi16sp" => rvgen::riscv::rv32c::rv32ic_addi16sp(a[0], a[1]),
            "rv32ic_addi4spn" => rvgen::riscv::rv32c::rv32ic_addi4spn(a[0], a[1]),
            "rv32ic_addi" => rvgen::riscv::rv32c::rv32ic_addi(a[0], a[1]),
            "rv32ic_li" => rvgen::riscv::rv32c::rv32ic_li(a[0], a[1]),
            "rv32ic_slli" => rvgen::riscv::rv32c::rv32ic_slli(a[0], a[1]),
            "rv32ic_andi" => rvgen::riscv::rv32c::rv32ic_andi(a[0], a[1]),
            "rv32ic_srli" => rvgen::riscv::rv32c::rv32ic_srli(a[0], a[1]),
            "rv32ic_srai" => rvgen::riscv::rv32c::rv32ic_srai(a[0], a[1]),
            "rv32ic_beqz" => rvgen::riscv::rv32c::rv32ic_beqz(a[0], a[1]),
            "rv32ic_bnez" => rvgen::riscv::rv32c::rv32ic_bnez(a[0], a[1]),
            "rv32ic_jal" => rvgen::riscv::rv32c::rv32ic_jal(a[0]),
            "rv32ic_j" => rvgen::riscv::rv32c::rv32ic_j(a[0]),
            "rv32ic_jr" => rvgen::riscv::rv32c::rv32ic_jr(a[0]),
            "rv32ic_jalr" => rvgen::riscv::rv32c::rv32ic_jalr(a[0]),
            "rv32ic_ebreak" => rvgen::riscv::rv32c::rv32ic_ebreak(),
            "rv32ic_lwsp" => rvgen::riscv::rv32c::rv32ic_lwsp(a[0], a[1]),
            "rv32ic_lw" => rvgen::riscv::rv32c::rv32ic_lw(a[0], a[1], a[2]),
            "rv32ic_swsp" => rvgen::riscv::rv32c::rv32ic_swsp(a[0], a[1]),
            "rv32ic_sw" => rvgen::riscv::rv32c::rv32ic_sw(a[0], a[1], a[2]),
            "rv64ic_addw" => rvgen::riscv::rv64c::rv64ic_addw(a[0], a[1]),
            "rv64ic_subw" => rvgen::riscv::rv64c::rv64ic_subw(a[0], a[1]),
            "rv64ic_addiw" => rvgen::riscv::rv64c::rv64ic_addiw(a[0], a[1]),
            "rv64ic_ldsp" => rvgen::riscv::rv64c::rv64ic_ldsp(a[0], a[1]),
            "rv64ic_ld" => rvgen::riscv::rv64c::rv64ic_ld(a[0], a[1], a[2]),
            "rv64ic_sdsp" => rvgen::riscv::rv64c::rv64ic_sdsp(a[0], a[1]),
            "rv64ic_sd" => rvgen::riscv::rv64c::rv64ic_sd(a[0], a[1], a[2]),
            "rvprivileged_sret" => rvgen::riscv::rvprivileged::rvprivileged_sret(),
            "rvprivileged_mret" => rvgen::riscv::rvprivileged::rvprivileged_mret(),
            "rvprivileged_wfi" => rvgen::riscv::rvprivileged::rvprivileged_wfi(),
            "rvprivileged_sfence_vma" => {
                rvgen::riscv::rvprivileged::rvprivileged_sfence_vma(a[0], a[1])
            }
            "rvprivileged_sinval_vma" => {
                rvgen::riscv::rvprivileged::rvprivileged_sinval_vma(a[0], a[1])
            }
            "rvprivileged_sfence_w_inval" => {
                rvgen::riscv::rvprivileged::rvprivileged_sfence_w_inval()
            }
            "rvprivileged_sfence_inval_ir" => {
                rvgen::riscv::rvprivileged::rvprivileged_sfence_inval_ir()
            }
            "zicsr_csrrw" => rvgen::riscv::zicsr::zicsr_csrrw(a[0], a[1], a[2]),
            "zicsr_csrrs" => rvgen::riscv::zicsr::zicsr_csrrs(a[0], a[1], a[2]),
            "zicsr_csrrc" => rvgen::riscv::zicsr::zicsr_csrrc(a[0], a[1], a[2]),
            "zicsr_csrrwi" => rvgen::riscv::zicsr::zicsr_csrrwi(a[0], a[1], a[2]),
            "zicsr_csrrsi" => rvgen::riscv::zicsr::zicsr_csrrsi(a[0], a[1], a[2]),
            "zicsr_csrrci" => rvgen::riscv::zicsr::zicsr_csrrci(a[0], a[1], a[2]),
            "zifencei_fencei" => rvgen::riscv::zifencei::zifencei_fencei(a[0]),
            name => panic!("unknown encoder {name}"),
        };
        let expected = u32::from_str_radix(fields[2], 16).unwrap();
        assert_eq!(actual, expected, "{}({:?})", fields[0], a);
        count += 1;
    }
    assert_eq!(count, 6784);
}
