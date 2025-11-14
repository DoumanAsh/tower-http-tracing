pub fn parse_grpc_status(bytes: &[u8]) -> u16 {
    match bytes.len() {
        1 => match bytes[0] {
            b'0' => 0,
            b'1' => 1,
            b'2' => 2,
            b'3' => 3,
            b'4' => 4,
            b'5' => 5,
            b'6' => 6,
            b'7' => 7,
            b'8' => 8,
            b'9' => 9,
            _ => 2 //unknown
        },
        2 => match (bytes[0], bytes[1]) {
            (b'1', b'0') => 10,
            (b'1', b'1') => 11,
            (b'1', b'2') => 12,
            (b'1', b'3') => 13,
            (b'1', b'4') => 14,
            (b'1', b'5') => 15,
            (b'1', b'6') => 16,
            _ => 2 //Unknown
        },
        _ => 2 // unknown
    }
}
