use core::arch::riscv64::sfence_vma;

use crate::mm::{
    MemorySet,
    PhysPageNum,
    KERNEL_SPACE, 
    VirtAddr,
    translated_refmut,
};
use crate::trap::{TrapContext, trap_handler};
use crate::config::{TRAP_CONTEXT};
use super::TaskContext;
use super::{PidHandle, pid_alloc, KernelStack,insert_into_pid2task, add_task};
use alloc::sync::{Weak, Arc};
use alloc::vec;
use alloc::vec::Vec;
use alloc::string::String;
use k210_pac::aes::en;
use spin::{Mutex, MutexGuard};
use crate::sync::{
    Mutex as MyMutex,
    Semaphore,
    Condvar,
};
use core::arch::asm;


use crate::fs::{File, Stdin, Stdout};
use super::{
    SignalFlags,
    SignalActions,
};

use crate::task::{
    ustack_bottom_from_pid,
    trap_cx_bottom_from_pid,
};

pub struct TaskControlBlock {
    // immutable
    pub pid: PidHandle,
    pub kernel_stack: KernelStack,
    pub tgid: usize,

    // mutable
    inner: Mutex<TaskControlBlockInner>,
}

pub struct TaskControlBlockInner {
    pub trap_cx_ppn: PhysPageNum,
    pub base_size: usize,
    pub task_cx: TaskContext,
    pub task_status: TaskStatus,
    pub memory_set: MemorySet,
    pub parent: Option<Weak<TaskControlBlock>>,
    pub children: Vec<Arc<TaskControlBlock>>,
    pub exit_code: Option<i32>,
    pub fd_table: Vec<Option<Arc<dyn File + Send + Sync>>>,
    pub signals: SignalFlags,
    pub signal_mask: SignalFlags,
    // the signal which is being handling
    pub handling_sig: isize,
    // Signal actions
    pub signal_actions: SignalActions,
    // if the task is killed
    pub killed: bool,
    // if the task is frozen by a signal
    pub frozen: bool,
    pub trap_ctx_backup: Option<TrapContext>,
    pub mutex_list: Vec<Option<Arc<dyn MyMutex>>>,
    pub semaphore_list: Vec<Option<Arc<Semaphore>>>,
    pub condvar_list: Vec<Option<Arc<Condvar>>>,
}

impl TaskControlBlockInner {
    pub fn get_task_cx_ptr(&mut self) -> *mut TaskContext {
        &mut self.task_cx as *mut TaskContext
    }
    pub fn get_trap_cx(&self) -> &'static mut TrapContext {
        self.trap_cx_ppn.get_mut()
    }
    pub fn get_user_token(&self) -> usize {
        self.memory_set.token()
    }
    fn get_status(&self) -> TaskStatus {
        self.task_status
    }
    pub fn is_zombie(&self) -> bool {
        self.get_status() == TaskStatus::Zombie
    }
    pub fn alloc_fd(&mut self) -> usize {
        if let Some(fd) = (0..self.fd_table.len()).find(|fd| self.fd_table[*fd].is_none()) {
            fd
        } else {
            self.fd_table.push(None);
            self.fd_table.len() - 1
        }
    }

    pub fn get_exit_code(&self) -> Option<i32> {
        self.exit_code
    }
}

impl TaskControlBlock {
    pub fn inner_exclusive_access(&self) -> MutexGuard<TaskControlBlockInner> {
        self.inner.lock()
    }

    pub fn trap_cx_user_va(&self) -> usize {
        let mut trap_cx_user_va = 0;

        if self.pid.0 == self.tgid {
            trap_cx_user_va = trap_cx_bottom_from_pid(0);
        }else{
            trap_cx_user_va = trap_cx_bottom_from_pid(self.pid.0);
        }
        trap_cx_user_va

    }    
    pub fn new(elf_data: &[u8]) -> Self {
        // alloc a pid 
        let pid_handle = pid_alloc();
        let pid = pid_handle.0;
        let tgid = pid_handle.0;
        // memory_set with elf program headers/trampoline/trap context/user stack
        
        use riscv::register::sstatus;
        let sstatus = sstatus::read();
        println!("");
        print!("before map-----------------sstatus = {:#0b}", sstatus.bits());
        println!("");

        let (memory_set, user_sp, entry_point) = MemorySet::from_elf(elf_data, 0);

        unsafe {
            asm!("sfence.vma");
        }
        // for tcb::new()   and tcb::exec()     
        // ustack trap_cx =  ustack_bottom_from_pid(0) trap_cx_bottom_from_pid(0)
        let trap_cx_bottom_va: VirtAddr = trap_cx_bottom_from_pid(0 as usize).into();
        let trap_cx_ppn = memory_set
            .translate(VirtAddr::from(trap_cx_bottom_va).into())
            .unwrap()
            .ppn();

        //alloc a kernel stack in kernel space
        let kernel_stack = KernelStack::new(&pid_handle);
        let kernel_stack_top = kernel_stack.get_top();
        let task_control_block = Self {
            pid: pid_handle,
            kernel_stack,
            tgid: tgid,
            inner: unsafe {
                Mutex::new(TaskControlBlockInner {
                    trap_cx_ppn,
                    base_size: user_sp,
                    task_cx: TaskContext::goto_trap_return(kernel_stack_top),
                    task_status: TaskStatus::Ready,
                    memory_set,
                    parent: None,
                    children: Vec::new(),
                    exit_code: None,
                    fd_table: vec![
                        // 0 -> stdin
                        Some(Arc::new(Stdin)),
                        // 1 -> stdout
                        Some(Arc::new(Stdout)),
                        // 2 -> stderr
                        Some(Arc::new(Stdout)),
                    ],
                    signals: SignalFlags::empty(),
                    signal_mask: SignalFlags::empty(),
                    handling_sig: -1,
                    signal_actions: SignalActions::default(),
                    killed: false,
                    frozen: false,
                    trap_ctx_backup: None,
                    mutex_list: Vec::new(),
                    semaphore_list: Vec::new(),
                    condvar_list: Vec::new(),
                })
            },
        };
        // prepare TrapContext in user space
        let trap_cx = task_control_block.inner_exclusive_access().get_trap_cx();
        *trap_cx = TrapContext::app_init_context(
            entry_point,
            user_sp,
            KERNEL_SPACE.exclusive_access().token(),
            kernel_stack_top,
            trap_handler as usize,
        );

        task_control_block
    }
    pub fn exec(&self, elf_data: &[u8], args: Vec<String>) {

        let parent_pid = self.pid.0;

        // memory_set with elf program headers/trampoline/trap context/user stack
        let (memory_set, mut user_sp, entry_point) = MemorySet::from_elf(elf_data, parent_pid);

        unsafe {
            asm!("sfence.vma");
        }
        let trap_cx_bottom_va: VirtAddr = trap_cx_bottom_from_pid(parent_pid).into();

        let trap_cx_ppn = memory_set
            .translate(VirtAddr::from(trap_cx_bottom_va).into())
            .unwrap()
            .ppn();
        // push arguments on user stack
        user_sp -= (args.len() + 1) * core::mem::size_of::<usize>();
        let argv_base = user_sp;
        let mut argv: Vec<_> = (0..=args.len())
            .map(|arg| {
                translated_refmut(
                    memory_set.token(),
                    (argv_base + arg * core::mem::size_of::<usize>()) as *mut usize,
                )
            })
            .collect();
        *argv[args.len()] = 0;
        for i in 0..args.len() {
            user_sp -= args[i].len() + 1;
            *argv[i] = user_sp;
            let mut p = user_sp;
            for c in args[i].as_bytes() {
                *translated_refmut(memory_set.token(), p as *mut u8) = *c;
                p += 1;
            }
            *translated_refmut(memory_set.token(), p as *mut u8) = 0;
        }
        // make the user_sp aligned to 8B for k210 platform
        user_sp -= user_sp % core::mem::size_of::<usize>();

        // **** access current TCB exclusively
        let mut inner = self.inner_exclusive_access();
        // substitute memory_set
        inner.memory_set = memory_set;
        // update trap_cx ppn
        inner.trap_cx_ppn = trap_cx_ppn;
        // initialize trap_cx
        let mut trap_cx = TrapContext::app_init_context(
            entry_point,
            user_sp,
            KERNEL_SPACE.exclusive_access().token(),
            self.kernel_stack.get_top(),
            trap_handler as usize,
        );
        trap_cx.x[10] = args.len();
        trap_cx.x[11] = argv_base;
        *inner.get_trap_cx() = trap_cx;
        // **** release current PCB
    }
    pub fn fork(self: &Arc<TaskControlBlock>) -> Arc<TaskControlBlock> {

        let parent_pid = self.pid.0;
        let pid_handle = pid_alloc();
        let pid = pid_handle.0;
        let tgid = parent_pid;

        // ---- hold parent PCB lock
        let mut parent_inner = self.inner_exclusive_access();

        // copy user space(include trap context)
        let (memory_set, user_sp) = MemorySet::from_existed_user(&parent_inner.memory_set, pid);

        unsafe {
            asm!("sfence.vma");
        }

        let trap_cx_bottom_va: VirtAddr = trap_cx_bottom_from_pid(pid as usize).into();

        let trap_cx_ppn = memory_set
            .translate(VirtAddr::from(trap_cx_bottom_va).into())
            .unwrap()
            .ppn();
        // alloc a pid and a kernel stack in kernel space

        // get parent pid

        let kernel_stack = KernelStack::new(&pid_handle);
        let kernel_stack_top = kernel_stack.get_top();
        // copy fd table
        let mut new_fd_table: Vec<Option<Arc<dyn File + Send + Sync>>> = Vec::new();
        for fd in parent_inner.fd_table.iter() {
            if let Some(file) = fd {
                new_fd_table.push(Some(file.clone()));
            } else {
                new_fd_table.push(None);
            }
        }
        let child = Arc::new(TaskControlBlock {
            pid: pid_handle,
            tgid: tgid,
            kernel_stack,
            inner: unsafe {
                Mutex::new(TaskControlBlockInner {
                    trap_cx_ppn,
                    base_size: user_sp,
                    task_cx: TaskContext::goto_trap_return(kernel_stack_top),
                    task_status: TaskStatus::Ready,
                    memory_set,
                    parent: Some(Arc::downgrade(self)),
                    children: Vec::new(),
                    exit_code: None,
                    fd_table: new_fd_table,
                    signals: SignalFlags::empty(),
                    // inherit the signal_mask and signal_action
                    signal_mask: parent_inner.signal_mask,
                    handling_sig: -1,
                    signal_actions: parent_inner.signal_actions.clone(),
                    killed: false,
                    frozen: false,
                    trap_ctx_backup: None,
                    mutex_list: Vec::new(),
                    semaphore_list: Vec::new(),
                    condvar_list: Vec::new(),
                })
            },
        });
        // add child
        parent_inner.children.push(child.clone());

        let p_cx = parent_inner.get_trap_cx();
        // modify kernel_sp in trap_cx
        // **** access child PCB exclusively
        let inner = child.inner_exclusive_access();
        let trap_cx = inner.get_trap_cx();
        
        *trap_cx = *parent_inner.get_trap_cx();
        trap_cx.kernel_sp = kernel_stack_top;
        
        drop(parent_inner);
        drop(inner);

        child
    }


    pub fn new_user_thread(self: &Arc<TaskControlBlock>, entry_point: usize, arg: usize, parent_pid:usize) -> Arc<TaskControlBlock> {

        // alloc a pid and a kernel stack in kernel space
        let pid_handle = pid_alloc();
        let pid = pid_handle.0;

        // let parent_pid = self.pid.0;
        let tgid = parent_pid;


        // ---- hold parent PCB lock
        let mut parent_inner = self.inner_exclusive_access();

        // copy user space(include trap context)
        let (memory_set, user_sp) = MemorySet::from_existed(&parent_inner.memory_set, pid);
        
        unsafe {
            asm!("sfence.vma");
        }
        let trap_cx_bottom_va: VirtAddr = trap_cx_bottom_from_pid(pid).into();
        let trap_cx_ppn = memory_set
            .translate(VirtAddr::from(trap_cx_bottom_va).into())
            .unwrap()
            .ppn();

        // get parent pid

        let kernel_stack = KernelStack::new(&pid_handle);
        let kernel_stack_top = kernel_stack.get_top();

        // copy fd table
        let mut new_fd_table: Vec<Option<Arc<dyn File + Send + Sync>>> = Vec::new();
        for fd in parent_inner.fd_table.iter() {
            if let Some(file) = fd {
                new_fd_table.push(Some(file.clone()));
            } else {
                new_fd_table.push(None);
            }
        }
        let task_control_block = Arc::new(TaskControlBlock {
            pid: pid_handle,
            tgid: tgid,
            kernel_stack,
            inner: unsafe {
                Mutex::new(TaskControlBlockInner {
                    trap_cx_ppn,
                    base_size: parent_inner.base_size,
                    task_cx: TaskContext::goto_trap_return(kernel_stack_top),
                    task_status: TaskStatus::Ready,
                    memory_set,
                    parent: Some(Arc::downgrade(self)),
                    children: Vec::new(),
                    exit_code: None,
                    fd_table: new_fd_table,
                    signals: SignalFlags::empty(),
                    // inherit the signal_mask and signal_action
                    signal_mask: parent_inner.signal_mask,
                    handling_sig: -1,
                    signal_actions: parent_inner.signal_actions.clone(),
                    killed: false,
                    frozen: false,
                    trap_ctx_backup: None,
                    mutex_list: Vec::new(),
                    semaphore_list: Vec::new(),
                    condvar_list: Vec::new(),
                })
            },
        });

        // add child
        parent_inner.children.push(task_control_block.clone());
        // modify kernel_sp in trap_cx

        // **** access child PCB exclusively
        let inner = task_control_block.inner_exclusive_access();

        let trap_cx = inner.get_trap_cx();
        
        trap_cx.kernel_sp = kernel_stack_top;
        
        
        let mut trap_cx = TrapContext::app_init_context(
            entry_point,
            user_sp,
            KERNEL_SPACE.exclusive_access().token(),
            kernel_stack_top,
            trap_handler as usize,
        );
        trap_cx.x[10] = arg;

        *inner.get_trap_cx() = trap_cx;

        drop(inner);
        // return   
        task_control_block
        // **** release child PCB
        // ---- release parent PCB
    }


    pub fn getpid(&self) -> usize {
        self.pid.0
    }
}


impl PartialEq for TaskControlBlock {
    fn eq(&self, other: &Self) -> bool {
        self.pid == other.pid
    }
}

impl Eq for TaskControlBlock {}

impl PartialOrd for TaskControlBlock {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for TaskControlBlock {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.pid.cmp(&other.pid)
    }
}



#[derive(Copy, Clone, PartialEq)]
pub enum TaskStatus {
    Ready,
    Running(usize),
    Zombie,
    Blocking,
}