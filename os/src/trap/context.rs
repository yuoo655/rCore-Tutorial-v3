use riscv::register::sstatus::{self, Sstatus, SPP};

#[repr(C)]
#[derive(Debug)]
pub struct TrapContext {
    pub x: [usize; 32],
    pub sstatus: Sstatus,
    pub sepc: usize,
    pub kernel_satp: usize,
    pub kernel_sp: usize,
    pub trap_handler: usize,
}

impl TrapContext {
    pub fn set_sp(&mut self, sp: usize) {
        self.x[2] = sp;
    }
    pub fn app_init_context(
        entry: usize,
        sp: usize,
        kernel_satp: usize,
        kernel_sp: usize,
        trap_handler: usize,
    ) -> Self {
        
        let mut sstatus = sstatus::read();
        // set CPU privilege to User after trapping back
        sstatus.set_spp(SPP::User);
        let mut cx = Self {
            x: [0; 32],
            sstatus,
            sepc: entry,
            kernel_satp,
            kernel_sp,
            trap_handler,
        };
        println!("app_init_context :{:#x?}", entry);
        cx.set_sp(sp);
        cx
    }


    pub fn kernel_init_context(entry: usize, sp:usize) -> Self {
        use crate::mm::kernel_token;
        use crate::trap::trap_handler;
    
        let mut sstatus = sstatus::read();
        sstatus.set_spp(SPP::Supervisor);

        let kernel_satp = kernel_token();
        let kernel_sp = sp;

        let mut cx = Self {
            x: [0; 32],
            sstatus,
            sepc: entry,
            kernel_satp,
            kernel_sp,
            trap_handler: trap_handler as usize,
        };
        cx.set_sp(sp);

        println!("kthread trap context addr:{:#x?}", entry);
        println!("kthread cx: {:x?}",cx);
        cx
    }
}
