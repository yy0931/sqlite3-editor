use std::rc::Rc;
use std::time::Duration;

use crate::cli_error::CLIError;
use crate::cli_subcommands::server::server_commands::query::sqlite3::columnar_buffer::ColumnarBuffer;
use crate::cli_value::CLIValue;
use crate::msgpack::encode_value_ref_into_msgpack;
use crate::msgpack::MessagePackArray;
use crate::utf8_extractor::InvalidUTF8;

use super::cache_entry::Records;
use super::pager_cache::PagerCache;

#[derive(Clone, Debug, PartialEq)]
pub struct PagerConfig {
    pub slow_query_threshold: Duration,
    pub per_query_cache_limit_bytes: u64,
    pub cache_time_limit_relative_to_queried_range: f64,
    pub cache_limit_bytes: u64,

    /// The number of records to prefetch before the start of the queried range.
    pub margin_start: u64,
    /// The number of records to prefetch after the end of the queried range.
    pub margin_end: u64,
}

impl Default for PagerConfig {
    fn default() -> Self {
        Self {
            slow_query_threshold: Duration::from_millis(500),
            per_query_cache_limit_bytes: /* 8MB */ 8 * 1024 * 1024,
            cache_time_limit_relative_to_queried_range: 0.2,
            cache_limit_bytes: /* 64MB */ 64 * 1024 * 1024,
            margin_start: 0,
            margin_end: 100000,
        }
    }
}

pub struct Pager {
    cache: PagerCache,
    data_version: Option<i64>,
    pub config: PagerConfig,

    #[cfg(test)]
    pub cache_hit_count: usize,

    #[cfg(test)]
    pub cache_clear_count: usize,

    #[cfg(test)]
    pub dequeue_count: usize,
}

impl Pager {
    pub fn new() -> Self {
        Self {
            cache: PagerCache::new(),
            data_version: None,
            config: PagerConfig::default(),
            #[cfg(test)]
            cache_hit_count: 0,
            #[cfg(test)]
            cache_clear_count: 0,
            #[cfg(test)]
            dequeue_count: 0,
        }
    }

    #[cfg(test)]
    pub fn data_version(&self) -> Option<i64> {
        self.data_version
    }

    pub fn clear_cache(&mut self) {
        self.cache.clear();
        #[cfg(test)]
        {
            self.cache_clear_count += 1;
        }
    }

    pub fn total_cache_size_bytes(&self) -> u64 {
        self.cache.total_size_bytes()
    }

    pub fn query<F: FnMut(InvalidUTF8)>(
        &mut self,
        conn: &mut rusqlite::Connection,
        query: &str,
        params: &[CLIValue],
        mut on_invalid_utf8: F,
    ) -> std::result::Result<Option<Records>, CLIError> {
        let mut params = params.to_vec();

        let tx = conn.transaction().or_else(|err| CLIError::new_query_error(err, "BEGIN;", &[]))?;
        let data_version = Some(pragma_data_version(&tx)?);
        if self.data_version != data_version {
            self.clear_cache();
            self.data_version = data_version;
        } else {
            while self.cache.total_size_bytes() > self.config.cache_limit_bytes {
                self.cache.dequeue();
                #[cfg(test)]
                {
                    self.dequeue_count += 1;
                }
            }
        }

        let len = params.len();
        if !query.ends_with("LIMIT ? OFFSET ?") || len < 2 {
            return Ok(None);
        }

        let (CLIValue::I64(limit), CLIValue::I64(offset)) = (&params[len - 2], &params[len - 1]) else {
            return Ok(None);
        };
        let (Ok(limit), Ok(offset)): (Result<u64, _>, Result<u64, _>) = ((*limit).try_into(), (*offset).try_into()) else {
            // Negative limits and negative offsets are not supported
            return Ok(None);
        };

        let cache_entry = self.cache.entry(query, &params);
        let mut cache_entry = cache_entry.borrow_mut();

        // cache hit
        if let Some(records) = cache_entry.get_range(offset, limit) {
            #[cfg(test)]
            {
                self.cache_hit_count += 1;
            }
            return Ok(Some(records));
        }

        // Add margins before and after the queried area
        let limit_with_margin = limit + self.config.margin_start.min(offset) + self.config.margin_end;
        params[len - 2] = CLIValue::I64(limit_with_margin.try_into().unwrap());
        let offset_with_margin = offset.saturating_sub(self.config.margin_start);
        params[len - 1] = CLIValue::I64(offset_with_margin.try_into().unwrap());

        // Forward run: Fetch the queried area and cache records after that
        let mut col_buf = ColumnarBuffer::default();
        let columns: Vec<String>;
        let mut end_margin_size = 0;
        {
            // Prepare
            let mut stmt = tx.prepare(query).or_else(|err| CLIError::new_query_error(err, query, &params))?;

            // Bind parameters
            for (i, param) in params.iter().enumerate() {
                stmt.raw_bind_parameter(i + 1, param)
                    .or_else(|err| CLIError::new_query_error(err, query, &params))?;
            }

            let cache_size_prev = cache_entry.total_size_bytes();

            // Fetch records
            let mut current_offset = offset_with_margin;
            let mut rows = stmt.raw_query();
            let timer = std::time::Instant::now();
            let mut elapsed_until_end_margin = None;
            loop {
                match rows.next() {
                    Ok(Some(row)) => {
                        let is_start_margin = current_offset < offset;
                        let is_end_margin = offset + limit <= current_offset;
                        let is_margin = is_start_margin || is_end_margin;

                        if is_end_margin {
                            if elapsed_until_end_margin.is_none() {
                                elapsed_until_end_margin = Some(timer.elapsed());
                            }
                            let elapsed_until_end_margin = elapsed_until_end_margin.unwrap();

                            if !(
                                // The query is slow and
                                elapsed_until_end_margin >= self.config.slow_query_threshold &&
                                // sqlite3_step()s in the end margin are fast and
                                (timer.elapsed() - elapsed_until_end_margin).div_f64(self.config.cache_time_limit_relative_to_queried_range) < elapsed_until_end_margin &&
                                // record sizes are small
                                cache_entry.total_size_bytes().saturating_sub(cache_size_prev) < self.config.per_query_cache_limit_bytes / 2
                            ) {
                                break;
                            }

                            end_margin_size += 1;
                        }

                        let mut cache_record = MessagePackArray::new();
                        // NOTE: We need to call `stmt.column_count()` after `rows.next()` (see https://github.com/rusqlite/rusqlite/blob/b7309f2dca70716fee44c85082c585b330edb073/src/column.rs#L51-L53),
                        //       but since the borrow checker prevents us from calling `stmt.column_count()` while `row` is alive,
                        //       we rely on `rusqlite::Error::InvalidColumnIndex` returned from `row.get_ref(i)` to check the number of columns.
                        for i in 0usize..=usize::MAX {
                            match row.get_ref(i) {
                                Ok(value) => {
                                    let value_msgpack = encode_value_ref_into_msgpack(value, &mut on_invalid_utf8);
                                    if !is_margin {
                                        col_buf.get_column(i).push_message_pack(value_msgpack.clone());
                                    }
                                    cache_record.push_message_pack(value_msgpack);
                                }
                                Err(rusqlite::Error::InvalidColumnIndex(_)) => break,
                                Err(err) => return CLIError::new_query_error(err, query, &params),
                            }
                        }
                        cache_entry.insert(current_offset, cache_record);

                        current_offset += 1;
                    }
                    Ok(None) => {
                        if current_offset < offset_with_margin + limit_with_margin {
                            cache_entry.set_num_records(current_offset);
                        }
                        break;
                    }
                    Err(err) => CLIError::new_query_error(err, query, &params)?,
                }
            }

            drop(rows);

            // NOTE: We need to call `stmt.column_names()` after `rows.next()` (see https://github.com/rusqlite/rusqlite/blob/b7309f2dca70716fee44c85082c585b330edb073/src/column.rs#L51-L53)
            columns = stmt.column_names().into_iter().map(|v| v.to_owned()).collect::<Vec<_>>();
            cache_entry.set_columns_if_not_set_yet(columns.clone());
        }

        // Backward run: cache `end_margin_size` records before the queried area
        // TODO: Send this work to another thread
        if end_margin_size > 0 {
            let backward_offset = offset.saturating_sub(end_margin_size);
            let backward_limit = offset.saturating_sub(backward_offset);
            params[len - 2] = CLIValue::I64(backward_limit.try_into().unwrap());
            params[len - 1] = CLIValue::I64(backward_offset.try_into().unwrap());
            let mut current_offset = backward_offset;
            if !cache_entry.has_range(backward_offset, backward_limit) {
                // Prepare
                let mut stmt = tx.prepare(query).or_else(|err| CLIError::new_query_error(err, query, &params))?;

                // Bind parametersnew_other_error
                for (i, param) in params.iter().enumerate() {
                    stmt.raw_bind_parameter(i + 1, param)
                        .or_else(|err| CLIError::new_query_error(err, query, &params))?;
                }

                // Fetch records
                let mut rows = stmt.raw_query();

                loop {
                    match rows.next() {
                        Ok(Some(row)) => {
                            let mut cache_record = MessagePackArray::new();
                            for i in 0..columns.len() {
                                cache_record.push_message_pack(encode_value_ref_into_msgpack(
                                    row.get_ref(i).or_else(|err| {
                                        CLIError::new_other_error(
                                            format!("Error while caching backwards, possibly due to the database schema being updated during the process: {err:?}"),
                                            Some(query.to_string()),
                                            Some(&params),
                                        )
                                    })?,
                                    &mut on_invalid_utf8,
                                ));
                            }
                            cache_entry.insert(current_offset, cache_record);

                            current_offset += 1;
                        }
                        Ok(None) => {
                            break;
                        }
                        Err(err) => CLIError::new_query_error(err, query, &params)?,
                    }
                }
            }
        }

        Ok(Some(Records::new(col_buf.finish(columns.len()), Rc::new(columns))))
    }
}

fn pragma_data_version(conn: &rusqlite::Connection) -> Result<i64, CLIError> {
    conn.pragma_query_value(None, "data_version", |row| row.get::<_, i64>(0))
        .or_else(|err| CLIError::new_query_error(err, "PRAGMA data_version", &[]))
}
