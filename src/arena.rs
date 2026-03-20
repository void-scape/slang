pub struct Arena {
    arena: *mut u8,
    capacity: usize,
    index: usize,
}

impl Arena {
    pub fn new(capacity: usize) -> Self {
        let layout = std::alloc::Layout::from_size_align(capacity, 8).unwrap();
        let arena = unsafe { std::alloc::alloc(layout) };
        Self {
            arena,
            capacity,
            index: 0,
        }
    }

    pub fn allocate<T: Copy>(&mut self, data: T) -> &'static T {
        let size = std::mem::size_of::<T>();
        let align = std::mem::align_of::<T>();
        let start = self.index.next_multiple_of(align);
        self.resize(start + size);
        self.index = start + size;
        unsafe {
            let dst = self.arena.add(start);
            std::ptr::copy_nonoverlapping(std::ptr::from_ref(&data).cast(), dst, size);
            std::mem::transmute::<&T, &'static T>(&*dst.cast::<T>())
        }
    }

    pub fn allocate_slice<T: Copy>(&mut self, slice: &[T]) -> &'static [T] {
        let len = slice.len();
        let bytes = std::mem::size_of_val(slice);
        let align = std::mem::align_of::<T>();
        let start = self.index.next_multiple_of(align);
        self.resize(start + bytes);
        self.index = start + bytes;
        unsafe {
            let dst = self.arena.add(start);
            std::ptr::copy_nonoverlapping(slice.as_ptr().cast(), dst, bytes);
            std::mem::transmute::<&[T], &'static [T]>(std::slice::from_raw_parts(
                dst.cast::<T>(),
                len,
            ))
        }
    }

    fn resize(&mut self, capacity: usize) {
        if self.capacity >= capacity {
            return;
        }
        self.capacity = (self.capacity * 2).max(capacity);
        let layout = std::alloc::Layout::from_size_align(self.capacity, 8).unwrap();
        self.arena = unsafe { std::alloc::alloc(layout) };
    }
}
