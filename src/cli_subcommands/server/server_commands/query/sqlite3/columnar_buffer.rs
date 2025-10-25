use crate::msgpack::MessagePackArray;

#[derive(Default)]
pub struct ColumnarBuffer {
    // column index -> a msgpack containing all the values in the column
    columns: Vec<MessagePackArray>,
}

impl ColumnarBuffer {
    pub fn get_column(&mut self, col_index: usize) -> &mut MessagePackArray {
        while self.columns.len() <= col_index {
            self.columns.resize_with(col_index + 1, MessagePackArray::new);
        }
        &mut self.columns[col_index]
    }

    pub fn finish(mut self, len: usize) -> Vec<MessagePackArray> {
        // resize for when there are no records
        self.columns.resize_with(len, MessagePackArray::new);
        self.columns
    }
}
