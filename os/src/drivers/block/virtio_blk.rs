use super::BlockDevice;
use crate::mm::{
    frame_alloc, frame_dealloc, kernel_token, FrameTracker, PageTable, PhysAddr, PhysPageNum,
    StepByOne, VirtAddr,
};
use crate::sync::{Condvar, UPIntrFreeCell};
use crate::task::schedule;
use crate::DEV_NON_BLOCKING_ACCESS;
use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use lazy_static::*;
use virtio_drivers::{BlkResp, RespStatus, VirtIOBlk, VirtIOHeader};
use lock::Mutex;

#[allow(unused)]
const VIRTIO0: usize = 0x10001000;

pub struct VirtIOBlock {
    virtio_blk: UPIntrFreeCell<VirtIOBlk<'static>>,
    condvars: BTreeMap<u16, Condvar>,
}

lazy_static! {
    static ref QUEUE_FRAMES: Mutex<Vec<FrameTracker>> = Mutex::new(Vec::new());
}

impl BlockDevice for VirtIOBlock {
    fn read_block(&self, block_id: usize, buf: &mut [u8]) {
        self.virtio_blk
            .exclusive_access()
            .read_block(block_id, buf)
            .expect("Error when reading VirtIOBlk");
    }
    fn write_block(&self, block_id: usize, buf: &[u8]) {
        self.virtio_blk
            .exclusive_access()
            .write_block(block_id, buf)
            .expect("Error when writing VirtIOBlk");
    }

    fn handle_irq(&self) {
        // self.0.exclusive_access().handle_irq();
    }
}





impl VirtIOBlock {
    pub fn new() -> Self {
        let virtio_blk = unsafe {
            UPIntrFreeCell::new(VirtIOBlk::new(&mut *(VIRTIO0 as *mut VirtIOHeader)).unwrap())
        };
        let mut condvars = BTreeMap::new();
        let channels = virtio_blk.exclusive_access().virt_queue_size();
        for i in 0..channels {
            let condvar = Condvar::new();
            condvars.insert(i, condvar);
        }
        Self {
            virtio_blk,
            condvars,
        }
    }
}

#[no_mangle]
pub extern "C" fn virtio_dma_alloc(pages: usize) -> PhysAddr {
    let mut ppn_base = PhysPageNum(0);
    for i in 0..pages {
        let frame = frame_alloc().unwrap();
        if i == 0 {
            ppn_base = frame.ppn;
        }
        assert_eq!(frame.ppn.0, ppn_base.0 + i);
        QUEUE_FRAMES.lock().push(frame);
    }
    ppn_base.into()
}

#[no_mangle]
pub extern "C" fn virtio_dma_dealloc(pa: PhysAddr, pages: usize) -> i32 {
    let mut ppn_base: PhysPageNum = pa.into();
    for _ in 0..pages {
        frame_dealloc(ppn_base);
        ppn_base.step();
    }
    0
}

#[no_mangle]
pub extern "C" fn virtio_phys_to_virt(paddr: PhysAddr) -> VirtAddr {
    VirtAddr(paddr.0)
}

#[no_mangle]
pub extern "C" fn virtio_virt_to_phys(vaddr: VirtAddr) -> PhysAddr {
    PageTable::from_token(kernel_token())
        .translate_va(vaddr)
        .unwrap()
}



