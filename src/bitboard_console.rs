use crate::bitboard::{BITBOD_WIDTH, Bitboard, FIELD_BOD_HEIGHT, FIELD_BOD_WIDTH, Move};
use regex::Regex;
use std::io;

fn read_buffer() -> String {
    let mut buffer = String::new();
    io::stdin()
        .read_line(&mut buffer)
        .expect("Failed to read line.");
    buffer.trim().to_string()
}

pub trait BitboardConsole {
    fn to_string(&self) -> String;
    fn read_to_move() -> Move;
}

impl BitboardConsole for Bitboard {
    fn to_string(&self) -> String {
        const COLOR_RESET: &str = "\u{001b}[0m";
        let mut buf = String::new();

        buf += "\n--------------------";
        buf += "\nnow turn player: ";
        buf += &self.turn_player.to_string();
        buf += "\n";

        buf += "  0 1 2 3 4\n";
        for c in 0..FIELD_BOD_HEIGHT {
            buf += &c.to_string();

            for r in 0..FIELD_BOD_WIDTH {
                buf += "\u{001b}[";
                buf += &(31 + ((self.player_bods[0] >> (c * BITBOD_WIDTH + r)) & 0b1)).to_string();
                buf += if (self.piece_bod >> (c * BITBOD_WIDTH + r)) & 0b1 == 0 {
                    r"m  "
                } else {
                    r"m● "
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
    fn read_to_move() -> Move {
        let set_re = Regex::new(r"S\s+(\d+)\s+(\d+)").unwrap();
        let flick_re = Regex::new(r"F\s+(\d+)\s+(\d+)\s+(\d)").unwrap();

        loop {
            let read_buf = read_buffer();

            match set_re.captures(&read_buf) {
                Some(caps) => {
                    return Move::Place {
                        r: caps[1].parse::<u64>().unwrap(),
                        c: caps[2].parse::<u64>().unwrap(),
                    };
                }
                None => (),
            }
            match flick_re.captures(&read_buf) {
                Some(caps) => {
                    return Move::Flick {
                        r: caps[1].parse::<u64>().unwrap(),
                        c: caps[2].parse::<u64>().unwrap(),
                        angle_idx: caps[3].parse::<usize>().unwrap(),
                    };
                }
                None => (),
            }
            println!(
                "コマンドの読み取りに失敗しました。\ncommands:\n    set y/x\n    flick y/x angle"
            );
        }
    }
}
