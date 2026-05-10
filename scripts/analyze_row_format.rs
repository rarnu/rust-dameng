fn bytes_to_hex(data: &[u8]) -> String {
    data.iter().map(|b| format!("{:02x}", b)).collect::<Vec<_>>().join(" ")
}

fn main() {
    // Actual hex dump from integration test (268 bytes)
    let data: Vec<u8> = vec![
        0x07, 0x00, 0x00, 0x00, 0x04, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00,
        // First col header (16 bytes)
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x02, 0x00, 0x03, 0x00,
        0x09, 0x00, 0x06, 0x00,
        // Col 1 strings
        0x49, 0x44, // "ID"
        0x49, 0x4e, 0x54, // "INT"
        0x52, 0x55, 0x53, 0x54, 0x5f, 0x54, 0x45, 0x53, 0x54, // "RUST_TEST"
        0x53, 0x59, 0x53, 0x44, 0x42, 0x41, // "SYSDBA"
        // Between (4 bytes)
        0x02, 0x00, 0x00, 0x00,
        // Reserved/padding (6 bytes?)
        0x90, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        // Col 2 header (24 bytes)
        0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x01, 0x00, 0x04, 0x00, 0x07, 0x00, 0x09, 0x00, 0x06, 0x00,
        // Col 2 strings
        0x4e, 0x41, 0x4d, 0x45, // "NAME"
        0x56, 0x41, 0x52, 0x43, 0x48, 0x41, 0x52, // "VARCHAR"
        0x52, 0x55, 0x53, 0x54, 0x5f, 0x54, 0x45, 0x53, 0x54, // "RUST_TEST"
        0x53, 0x59, 0x53, 0x44, 0x42, 0x41, // "SYSDBA"
        // Between (4 bytes)
        0x07, 0x00, 0x00, 0x00,
        // Col 3 header (24 bytes)
        0x04, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x03, 0x00, 0x03, 0x00, 0x09, 0x00,
        0x06, 0x00,
        // Col 3 strings
        0x41, 0x47, 0x45, // "AGE"
        0x49, 0x4e, 0x54, // "INT"
        0x52, 0x55, 0x53, 0x54, 0x5f, 0x54, 0x45, 0x53, 0x54, // "RUST_TEST"
        0x53, 0x59, 0x53, 0x44, 0x42, 0x41, // "SYSDBA"
        // Row 1
        0x23, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x10, 0x00, 0x16, 0x00, 0x1d, 0x00,
        0x04, 0x00, 0x01, 0x00, 0x00, 0x00,
        0x05, 0x00, 0x41, 0x6c, 0x69, 0x63, 0x65,
        0x04, 0x00, 0x19, 0x00, 0x00, 0x00,
        // Row 2
        0x21, 0x00, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x10, 0x00, 0x16, 0x00, 0x1b, 0x00,
        0x04, 0x00, 0x02, 0x00, 0x00, 0x00,
        0x03, 0x00, 0x42, 0x6f, 0x62,
        0x04, 0x00, 0x1e, 0x00, 0x00, 0x00,
        // Row 3
        0x25, 0x00, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x10, 0x00, 0x16, 0x00, 0x1f, 0x00,
        0x04, 0x00, 0x03, 0x00, 0x00, 0x00,
        0x07, 0x00, 0x43, 0x68, 0x61, 0x72, 0x6c, 0x69, 0x65,
        0x04, 0x00, 0x23, 0x00, 0x00, 0x00,
    ];

    println!("Total bytes: {}\n", data.len());

    // ===== HEADER (32 bytes) =====
    let sub_type = data[0];
    let header_row_count = u32::from_le_bytes([data[12], data[13], data[14], data[15]]);
    let first_col_type = i32::from_le_bytes([data[16], data[17], data[18], data[19]]);
    let first_nullable = u16::from_le_bytes([data[20], data[21]]);
    let first_display = u16::from_le_bytes([data[22], data[23]]);
    let first_col_name_len = u16::from_le_bytes([data[24], data[25]]);
    let first_type_name_len = u16::from_le_bytes([data[26], data[27]]);
    let first_table_len = u16::from_le_bytes([data[28], data[29]]);
    let first_schema_len = u16::from_le_bytes([data[30], data[31]]);

    println!("=== HEADER ===");
    println!("  sub_type={}, row_count={}, col_type={}, nullable={}, display={}",
        sub_type, header_row_count, first_col_type, first_nullable, first_display);
    println!("  col_name_len={}, type_name_len={}, table_len={}, schema_len={}",
        first_col_name_len, first_type_name_len, first_table_len, first_schema_len);

    let mut offset = 32;

    // ===== COLUMN 1 =====
    let c1_name = String::from_utf8_lossy(&data[offset..offset+first_col_name_len as usize]);
    offset += first_col_name_len as usize;
    let c1_type = String::from_utf8_lossy(&data[offset..offset+first_type_name_len as usize]);
    offset += first_type_name_len as usize;
    let c1_table = String::from_utf8_lossy(&data[offset..offset+first_table_len as usize]);
    offset += first_table_len as usize;
    let c1_schema = String::from_utf8_lossy(&data[offset..offset+first_schema_len as usize]);
    offset += first_schema_len as usize;
    println!("\n=== COL 1 === name='{}' type='{}' table='{}' schema='{}' offset={}",
        c1_name, c1_type, c1_table, c1_schema, offset);

    // ===== BETWEEN + COL 2 =====
    let between = bytes_to_hex(&data[offset..offset+4]);
    offset += 4;
    let extra = bytes_to_hex(&data[offset..offset+6]); // the 0x90 0x01... bytes
    offset += 6;
    println!("  BETWEEN={}", between);
    println!("  EXTRA({}) bytes: {}", 6, extra);

    let c2_type = i32::from_le_bytes([data[offset], data[offset+1], data[offset+2], data[offset+3]]);
    let c2_nullable = u16::from_le_bytes([data[offset+4], data[offset+5]]);
    let c2_display = u16::from_le_bytes([data[offset+6], data[offset+7]]);
    let c2_reserved = bytes_to_hex(&data[offset+8..offset+12]);
    let c2_index = u16::from_le_bytes([data[offset+12], data[offset+13]]);
    let c2_name_len = u16::from_le_bytes([data[offset+14], data[offset+15]]);
    let c2_type_name_len = u16::from_le_bytes([data[offset+16], data[offset+17]]);
    let c2_table_len = u16::from_le_bytes([data[offset+18], data[offset+19]]);
    let c2_schema_len = u16::from_le_bytes([data[offset+20], data[offset+21]]);
    println!("  COL2 header: type={} nullable={} display={} reserved={} index={} name_len={} type_name_len={} table_len={} schema_len={}",
        c2_type, c2_nullable, c2_display, c2_reserved, c2_index, c2_name_len, c2_type_name_len, c2_table_len, c2_schema_len);
    offset += 24;

    let c2_name = String::from_utf8_lossy(&data[offset..offset+c2_name_len as usize]);
    offset += c2_name_len as usize;
    let c2_type_name = String::from_utf8_lossy(&data[offset..offset+c2_type_name_len as usize]);
    offset += c2_type_name_len as usize;
    let c2_table = String::from_utf8_lossy(&data[offset..offset+c2_table_len as usize]);
    offset += c2_table_len as usize;
    let c2_schema = String::from_utf8_lossy(&data[offset..offset+c2_schema_len as usize]);
    offset += c2_schema_len as usize;
    println!("  COL 2: name='{}' type='{}' table='{}' schema='{}' offset={}",
        c2_name, c2_type_name, c2_table, c2_schema, offset);

    // ===== BETWEEN + COL 3 =====
    let between3 = bytes_to_hex(&data[offset..offset+4]);
    offset += 4;
    println!("  BETWEEN={}", between3);

    let c3_type = i32::from_le_bytes([data[offset], data[offset+1], data[offset+2], data[offset+3]]);
    let c3_nullable = u16::from_le_bytes([data[offset+4], data[offset+5]]);
    let c3_display = u16::from_le_bytes([data[offset+6], data[offset+7]]);
    let c3_reserved = bytes_to_hex(&data[offset+8..offset+12]);
    let c3_index = u16::from_le_bytes([data[offset+12], data[offset+13]]);
    let c3_name_len = u16::from_le_bytes([data[offset+14], data[offset+15]]);
    let c3_type_name_len = u16::from_le_bytes([data[offset+16], data[offset+17]]);
    let c3_table_len = u16::from_le_bytes([data[offset+18], data[offset+19]]);
    let c3_schema_len = u16::from_le_bytes([data[offset+20], data[offset+21]]);
    println!("  COL3 header: type={} nullable={} display={} reserved={} index={} name_len={} type_name_len={} table_len={} schema_len={}",
        c3_type, c3_nullable, c3_display, c3_reserved, c3_index, c3_name_len, c3_type_name_len, c3_table_len, c3_schema_len);
    offset += 24;

    let c3_name = String::from_utf8_lossy(&data[offset..offset+c3_name_len as usize]);
    offset += c3_name_len as usize;
    let c3_type_name = String::from_utf8_lossy(&data[offset..offset+c3_type_name_len as usize]);
    offset += c3_type_name_len as usize;
    let c3_table = String::from_utf8_lossy(&data[offset..offset+c3_table_len as usize]);
    offset += c3_table_len as usize;
    let c3_schema = String::from_utf8_lossy(&data[offset..offset+c3_schema_len as usize]);
    offset += c3_schema_len as usize;
    println!("  COL 3: name='{}' type='{}' table='{}' schema='{}' offset={}",
        c3_name, c3_type_name, c3_table, c3_schema, offset);

    println!("\n=== ROW DATA STARTS AT OFFSET {} ({} remaining bytes) ===", offset, data.len() - offset);

    // ===== ROWS =====
    let col_count = 3;
    let mut row_num = 0;
    while offset < data.len() {
        let row_start = offset;
        let row_size = data[offset] as usize;
        let flags = data[offset + 1];
        let rec_id = u32::from_le_bytes([data[offset+2], data[offset+3], data[offset+4], data[offset+5]]);

        println!("\nRow {} (abs={}): size={} flags=0x{:02x} rec_id={}", row_num, row_start, row_size, flags, rec_id);
        println!("  Header(10): {}", bytes_to_hex(&data[row_start..row_start+10]));

        let offsets_table = row_start + 10;
        println!("  Offsets table: {}", bytes_to_hex(&data[offsets_table..offsets_table+col_count*2]));

        // Column offsets (relative to row_start)
        let col_offsets: Vec<u16> = (0..col_count).map(|c| {
            u16::from_le_bytes([data[offsets_table + c*2], data[offsets_table + c*2 + 1]])
        }).collect();

        for (ci, &coff) in col_offsets.iter().enumerate() {
            let val_abs = row_start + coff as usize;
            if val_abs + 2 <= data.len() {
                let val_size = u16::from_le_bytes([data[val_abs], data[val_abs+1]]) as usize;
                let val_bytes = if val_size > 0 && val_abs + 2 + val_size <= row_start + row_size {
                    &data[val_abs+2..val_abs+2+val_size]
                } else {
                    &[]
                };
                let val_str = String::from_utf8_lossy(val_bytes);
                let val_hex = bytes_to_hex(val_bytes);
                println!("  Col {} (rel_off={}, abs={}): size={} text='{}' hex={}",
                    ci, coff, val_abs, val_size, val_str, val_hex);
            }
        }

        println!("  Row spans bytes {}-{}", row_start, row_start + row_size - 1);
        println!("  Row raw: {}", bytes_to_hex(&data[row_start..row_start+row_size]));

        offset += row_size;
        row_num += 1;
    }

    println!("\nTotal rows parsed: {}", row_num);
    if offset < data.len() {
        println!("Trailing: {}", bytes_to_hex(&data[offset..]));
    } else {
        println!("No trailing bytes - exact match!");
    }
}
