#[derive(Default)]
pub struct ColumnarBuffer {
    // column index -> a msgpack containing all the values in the column
    columns: Vec<Vec<u8>>,
}

impl ColumnarBuffer {
    pub fn get_column(&mut self, col_index: usize) -> &mut Vec<u8> {
        while self.columns.len() <= col_index {
            self.columns.resize(col_index + 1, vec![]);
        }
        &mut self.columns[col_index]
    }

    pub fn finish(mut self, len: usize) -> Vec<Vec<u8>> {
        // resize for when there are no records
        self.columns.resize(len, vec![]);
        self.columns
    }
}
