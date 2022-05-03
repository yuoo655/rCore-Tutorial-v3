use buddy_system_allocator::LockedHeap;
use core::{mem::MaybeUninit};

const HEAP_SIZE: usize = 0x800000;
static HEAP_MEMORY: MaybeUninit<[u8; HEAP_SIZE]> = core::mem::MaybeUninit::uninit();

#[global_allocator]
static HEAP: LockedHeap = LockedHeap::empty();


#[no_mangle]
pub unsafe fn init_heap_allocator() {
    let heap_start = HEAP_MEMORY.as_ptr() as usize;
    HEAP.lock().init(heap_start, HEAP_SIZE);
}

#[alloc_error_handler]
pub fn handle_alloc_error(layout: core::alloc::Layout) -> ! {
    panic!("Heap allocation error, layout = {:?}", layout);
}
