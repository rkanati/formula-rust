
const IDX_BITS: usize = 13;
const LEN_BITS: usize =  4;
const WIN_LEN: usize = 1 << IDX_BITS;
const BREAKEVEN: usize = (IDX_BITS + LEN_BITS + 1) / 9;
const EOSTREAM: usize = 0;

pub fn expand(bs: &[u8]) -> Vec<u8> {
    let mut read_bits = {
        let mut bs = bs.iter().copied();
        let mut in_mask = 0x80;
        let mut in_rack = 0;

        move |n| {
            let mut mask = 1u32 << (n - 1);
            let mut value = 0u32;
            while mask != 0 {
                if in_mask == 0x80 {in_rack = bs.next().unwrap() as u32;}
                if (in_mask & in_rack) != 0 {value |= mask;}
                mask >>= 1;
                in_mask >>= 1;
                if in_mask == 0 {in_mask = 0x80}
            }
            value
        }
    };

    let mut cur_pos = 1;
    let mut out = Vec::new();
    let mut window = [0u8; WIN_LEN];

    loop {
        let literal = read_bits(1) != 0;
        if literal {
            let byte = read_bits(8) as u8;
            out.push(byte);
            window[cur_pos] = byte;
            cur_pos = (cur_pos + 1) % window.len();
        }
        else {
            let pos = read_bits(IDX_BITS) as usize;
            if pos == EOSTREAM {break out}
            let len = read_bits(LEN_BITS) as usize + BREAKEVEN;

            for i in 0 ..= len {
                let out_byte = window[(pos + i) % window.len()];
                out.push(out_byte);
                window[cur_pos] = out_byte;
                cur_pos = (cur_pos + 1) % window.len();
            }
        }
    }
}

