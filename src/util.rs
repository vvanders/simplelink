///! Utility functions

pub trait CollectSlice<T>: Iterator<Item=T> {
    fn collect_slice(&mut self, out_slice: &mut [T]) {
        self.collect_slice_offset(out_slice, 0);
    }

    fn collect_slice_offset(&mut self, out_slice: &mut [T], offset: usize) {
        let mut idx = 0;
        for item in self.skip(offset) {
            out_slice[idx] = item;
            idx += 1;
        }
    }
}

impl<I: Iterator<Item=T>, T> CollectSlice<T> for I {}