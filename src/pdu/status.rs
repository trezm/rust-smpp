#[repr(u32)]
#[allow(non_camel_case_types)]
pub enum PduStatus {
    ESME_ROK = 0x00000000,
    ESME_RINVMSGLEN = 0x00000001,
    ESME_RINVCMDLEN = 0x00000002,
    ESME_RINVCMDID = 0x00000003,
    ESME_RINVBNDSTS = 0x00000004,
    ESME_RALYBND = 0x00000005,
    ESME_RINVPRTFLG = 0x00000006,
    ESME_RINVREGDLVFLG = 0x00000007,
    ESME_RSYSERR = 0x00000008,
    // Reserved: 0x00000009,
    ESME_RINVSRCADR = 0x0000000A,
    ESME_RINVDSTADR = 0x0000000B,
    ESME_RINVMSGID = 0x0000000C,
    ESME_RBINDFAIL = 0x0000000D,
    ESME_RINVPASWD = 0x0000000E,
    ESME_RINVSYSID = 0x0000000F,
    // Reserved: 0x00000010,
    ESME_RCANCELFAIL = 0x00000011,
    // Reserved: 0x00000012,
    ESME_RREPLACEFAIL = 0x00000013,
    ESME_RMSGQFUL = 0x00000014,
    ESME_RINVSERTYP = 0x00000015,
    // Reserved: 0x00000016-0x00000032,
    ESME_RINVNUMDESTS = 0x00000033,
    ESME_RINVDLNAME = 0x00000034,
    // Reserved: 0x00000035-0x0000003F,
    ESME_RINVDESTFLAG = 0x00000040,
    // Reserved: 0x00000041,
    ESME_RINVSUBREP = 0x00000042,
    ESME_RINVESMCLASS = 0x00000043,
    ESME_RCNTSUBDL = 0x00000044,
    ESME_RSUBMITFAIL = 0x00000045,
    // Reserved: 0x00000046-0x00000047,
    ESME_RINVSRCTON = 0x00000048,
    ESME_RINVSRCNPI = 0x00000049,
    ESME_RINVDSTTON = 0x00000050,
    ESME_RINVDSTNPI = 0x00000051,
    // Reserved: 0x00000052,
    ESME_RINVSYSTYP = 0x00000053,
    ESME_RINVREPFLAG = 0x00000054,
    ESME_RINVNUMMSGS = 0x00000055,
    // Reserved: 0x00000056-0x00000057,
    ESME_RTHROTTLED = 0x00000058,
    // Reserved, 0x00000059-0x00000060,
    ESME_RINVSCHED = 0x00000061,
    ESME_RINVEXPIRY = 0x00000062,
    ESME_RINVDFTMSGID = 0x00000063,
    ESME_RX_T_APPN = 0x00000064,
    ESME_RX_P_APPN = 0x00000065,
    ESME_RX_R_APPN = 0x00000066,
    ESME_RQUERYFAIL = 0x00000067,
    // Reserved: 0x00000068-0x000000BF,
    ESME_RINVOPTPARSTREAM = 0x000000C0,
    ESME_ROPTPARNOTALLWD = 0x000000C1,
    ESME_RINVPARLEN = 0x000000C2,
    ESME_RMISSINGOPTPARAM = 0x000000C3,
    ESME_RINVOPTPARAMVAL = 0x000000C4,
    // Reserved: 0x000000C5-0x000000FD,
    ESME_RDELIVERYFAILURE = 0x000000FE,
    ESME_RUNKNOWNERR = 0x000000FF,
    // Reserved for SMPP extension: 0x00000100-0x000003FF
    // Reserved for SMSC vendor specific errors: 0x00000400-0x000004FF
    // Reserved: 0x00000500-0xFFFFFFFF,
}
