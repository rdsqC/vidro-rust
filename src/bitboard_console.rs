use crate::bitboard::{BITBOD_WIDTH, Bitboard, FIELD_BOD_HEIGHT, FIELD_BOD_WIDTH, MoveBit};
use regex::Regex;
use std::io;

fn read_buffer() -> String {
    let mut buffer = String::new();
    io::stdin()
        .read_line(&mut buffer)
        .expect("Failed to read line.");
    buffer.trim().to_string()
}

fn format_with_underscores(n: u64) -> String {
    let mut result = String::new();
    let interval = BITBOD_WIDTH;
    let mut count = 64 % BITBOD_WIDTH;
    let mask = 1u64 << 63;

    for i in 0..64 {
        if count == 0 {
            count = interval;
            result.push('_');
        }
        if (n << i) & mask == 0 {
            result += &"0";
        } else {
            result += &"1";
        }
        count -= 1;
    }
    result
}

pub trait BitboardConsole {
    fn to_string(&self) -> String;
    fn read_to_move() -> MoveBit;
    fn print_data(&self) -> ();
    fn print_u64(title: &str, n: u64) -> ();
}

impl BitboardConsole for Bitboard {
    fn to_string(&self) -> String {
        const COLOR_RESET: &str = "\u{001b}[0m";
        let mut buf = String::new();

        buf += "\n--------------------";
        buf += "\nnow turn player: ";
        buf += &((-self.turn + 1) / 2).to_string();
        buf += "\n";

        buf += "  0 1 2 3 4\n";
        for c in 0..FIELD_BOD_HEIGHT {
            buf += &c.to_string();

            for r in 0..FIELD_BOD_WIDTH {
                buf += "\u{001b}[";
                buf += &(31 + ((self.player_bods[1] >> (c * BITBOD_WIDTH + r)) & 0b1)).to_string();
                buf += if ((self.player_bods[0] | self.player_bods[1]) >> (c * BITBOD_WIDTH + r))
                    & 0b1
                    == 0
                {
                    r"m  "
                } else {
                    r"m ●"
                };
                buf += COLOR_RESET;
            }
            buf += "\n";
        }

        for i in 0..2 {
            buf += "player";
            buf += &i.to_string();
            buf += ": ";
            buf += &self.have_piece[i as usize].to_string();
            buf += "\n";
        }

        return buf;
    }
    fn read_to_move() -> MoveBit {
        let set_re = Regex::new(r"[sS]\s+(\d+)\s+(\d+)").unwrap();
        let flick_re = Regex::new(r"[fF]\s+(\d+)\s+(\d+)\s+(\d)").unwrap();

        loop {
            let read_buf = read_buffer();

            match set_re.captures(&read_buf) {
                Some(caps) => {
                    return MoveBit::new(
                        caps[1].parse::<u8>().unwrap(),
                        caps[2].parse::<u8>().unwrap(),
                        8,
                    );
                }
                None => (),
            }
            match flick_re.captures(&read_buf) {
                Some(caps) => {
                    return MoveBit::new(
                        caps[1].parse::<u8>().unwrap(),
                        caps[2].parse::<u8>().unwrap(),
                        caps[3].parse::<u8>().unwrap(),
                    );
                }
                None => (),
            }
            println!(
                "コマンドの読み取りに失敗しました。\ncommands:\n    S c r\n    F c r angle_idx"
            );
        }
    }
    fn print_data(&self) {
        const COLOR_RESET: &str = "\u{001b}[0m";
        let mut buf = String::new();

        buf += "\n----------debug_data----------";
        buf += "\nfinal display:";
        buf += "\n--------------------";
        buf += "\nnow turn player: ";
        buf += &((-self.turn + 1) / 2).to_string();
        buf += "\n";

        buf += "  0 1 2 3 4\n";
        for c in 0..FIELD_BOD_HEIGHT {
            buf += &c.to_string();

            for r in 0..BITBOD_WIDTH {
                buf += "\u{001b}[";
                buf += &(31 + ((self.player_bods[1] >> (c * BITBOD_WIDTH + r)) & 0b1)).to_string();
                buf += if ((self.player_bods[0] | self.player_bods[1]) >> (c * BITBOD_WIDTH + r))
                    & 0b1
                    == 0
                {
                    r"m  "
                } else {
                    r"m ●"
                };
                buf += COLOR_RESET;
            }
            buf += "\n";
        }

        for i in 0..2 {
            buf += "player";
            buf += &i.to_string();
            buf += ": ";
            buf += &self.have_piece[i as usize].to_string();
            buf += "\n";
        }

        buf += "\nself.piece_bod\n";
        buf += "  0 1 2 3 4\n";
        buf += COLOR_RESET;
        for c in 0..FIELD_BOD_HEIGHT {
            buf += &c.to_string();

            for r in 0..BITBOD_WIDTH {
                buf += if ((self.player_bods[0] | self.player_bods[1]) >> (c * BITBOD_WIDTH + r))
                    & 0b1
                    == 0
                {
                    r"  "
                } else {
                    r" ●"
                };
            }
            buf += "\n";
        }
        buf += &format!(
            "binaly: {}",
            format_with_underscores(self.player_bods[0] | self.player_bods[1])
        );

        buf += "\nself.turn_player[0]\n";
        buf += "  0 1 2 3 4\n";
        for c in 0..FIELD_BOD_HEIGHT {
            buf += &c.to_string();

            for r in 0..BITBOD_WIDTH {
                buf += "\u{001b}[31m";
                buf += if (self.player_bods[0] >> (c * BITBOD_WIDTH + r)) & 0b1 == 0 {
                    r"  "
                } else {
                    r" ●"
                };
                buf += COLOR_RESET;
            }
            buf += "\n";
        }
        buf += &format!("binaly: {}", format_with_underscores(self.player_bods[0]));

        buf += "\nself.turn_player[1]\n";
        buf += "  0 1 2 3 4\n";
        for c in 0..FIELD_BOD_HEIGHT {
            buf += &c.to_string();

            for r in 0..BITBOD_WIDTH {
                buf += "\u{001b}[32m";
                buf += if (self.player_bods[1] >> (c * BITBOD_WIDTH + r)) & 0b1 == 0 {
                    r"  "
                } else {
                    r" ●"
                };
                buf += COLOR_RESET;
            }
            buf += "\n";
        }
        buf += &format!("binaly: {}", format_with_underscores(self.player_bods[1]));
        buf += &format!("\nbit_board: {:#?}", self);

        println!("{}", buf);
    }
    fn print_u64(title: &str, n: u64) -> () {
        let mut buf = String::new();
        buf += title;
        buf += ":\n";
        for c in 0..FIELD_BOD_HEIGHT {
            buf += &c.to_string();

            for r in 0..BITBOD_WIDTH {
                buf += if (n >> (c * BITBOD_WIDTH + r)) & 0b1 == 0 {
                    r"  "
                } else {
                    r" ●"
                };
            }
            buf += "\n";
        }
        buf += &format!("binaly: {}", format_with_underscores(n));
        println!("{}", buf);
    }
}
