#![allow(dead_code)]
#![allow(unused_imports)]

#[macro_use]
extern crate nom;

extern crate crc;

use nom::{HexDisplay, Needed, IResult, ErrorKind, le_i32, le_u64, le_u32, le_u8, le_u16, length_value, le_f32,
          FileProducer};
use nom::Err;
use nom::IResult::*;

struct A {
    a: u8,
    b: u8,
}

#[derive(PartialEq,Debug)]
enum RProp {
    Array(Vec<Vec<(String, RProp)>>),
    Bool(bool),
    Byte(u8),
    Float(f32),
    Int(u32),
    Name(String),
    QWord(u64),
    Str(String),
}

named!(length_encoded,
       chain!(
        size: le_u32 ~
        crc: le_u32 ~
        data: take!(size),
        || {data}
    ));

/// Text is encoded with a leading int that denotes the number of bytes that
/// the text spans. The last byte in the text will be null terminated, so we trim
/// it off. It may seem redundant to store this information, but stackoverflow contains
/// a nice reasoning for why it may have been done this way:
///
/// http://stackoverflow.com/questions/6293457/why-are-c-net-strings-length-prefixed-and-null-terminated
named!(text_encoded<&[u8], &str>,
    chain!(
        size: le_u32 ~
        data: take_str!(size - 1) ~
        take!(1),
        || {data}
    )
);

named!(str_prop<&[u8], RProp>,
  chain!(le_u64 ~ x: text_encoded,
    || {RProp::Str(x.to_string())}));

named!(name_prop<&[u8], RProp>,
  chain!(le_u64 ~ x: text_encoded,
    || {RProp::Name(x.to_string())}));

named!(int_prop<&[u8], RProp>,
    chain!(le_u64 ~ x: le_u32,
        || {RProp::Int(x)}));

named!(bool_prop<&[u8], RProp>,
    chain!(le_u64 ~ x: le_u8,
        || {RProp::Bool(x == 1)}));

named!(float_prop<&[u8], RProp>,
    chain!(le_u64 ~ x: le_f32,
        || {RProp::Float(x)}));

named!(qword_prop<&[u8], RProp>,
    chain!(le_u64 ~ x: le_u64,
        || {RProp::QWord(x)}));

named!(array_prop<&[u8], RProp>,
    chain!(
        le_u64 ~
        size: le_u32 ~
        elems: count!(rdict, size as usize),
        || {RProp::Array(elems)}));

named!(rprop_encoded<&[u8], RProp>,
  switch!(text_encoded,
    "ArrayProperty" => call!(array_prop) |
    "BoolProperty" => call!(bool_prop) |
    "ByteProperty" => call!(str_prop)|
    "FloatProperty" => call!(float_prop) |
    "IntProperty" => call!(int_prop) |
    "NameProperty" => call!(name_prop) |
    "QWordProperty" => call!(qword_prop) |
    "StrProperty" => call!(str_prop)
  )
);

fn rdict(input: &[u8]) -> IResult<&[u8], Vec<(String, RProp)> > {
    let mut v: Vec<(String, RProp)> = Vec::new();
    let mut res: IResult<&[u8], Vec<(String, RProp)>> = IResult::Done(input, Vec::new());
    let mut done = false;
    let mut cslice = input;

    while !done {
      match text_encoded(cslice) {
        IResult::Done(i, txt) => {
          cslice = i;
          match txt {
            "None" => { done = true }
            _ => {
              match rprop_encoded(cslice) {
                IResult::Done(inp, val) => { cslice = inp; v.push((txt.to_string(), val)); },
                IResult::Incomplete(a) => { res = IResult::Incomplete(a); done = true },
                IResult::Error(a) => { res = IResult::Error(a); done = true }
              }
            }
          }
        },

        IResult::Incomplete(a) => {
          done = true;
          res = IResult::Incomplete(a);
        },

        IResult::Error(a) => {
          done = true;
          res = IResult::Error(a);
        }
      }
    }

    match res {
      IResult::Done(a, b) => IResult::Done(cslice, v),
      _ => res
    }
}

named!(f<&[u8],A>,
    chain!(
        header_size:  le_i32 ~
        header_crc:   le_i32 ~
        header:       take!(header_size) ~
        content_size: le_i32 ~
        content_crc:  le_i32 ~
        content:      take!(content_size),
        || {A {a: 0, b: 0}}
    )
);


#[cfg(test)]
mod tests {
    use nom::IResult::{Done, Error, Incomplete};
    use nom::Needed::Size;
    use super::RProp::*;

    #[test]
    fn missing_header_data() {
        let data = include_bytes!("../assets/rumble.replay");
        let r = super::length_encoded(&data[..8]);
        assert_eq!(r, Incomplete(Size(4776)));
    }

    #[test]
    fn incomplete_header_data() {
        let data = include_bytes!("../assets/rumble.replay");
        let r = super::length_encoded(&data[..9]);
        assert_eq!(r, Incomplete(Size(4776)));
    }

    #[test]
    fn missing_header() {
        let r = super::length_encoded(&[]);
        assert_eq!(r, Incomplete(Size(4)));
    }

    #[test]
    fn missing_crc_data() {
        let data = include_bytes!("../assets/rumble.replay");
        let r = super::length_encoded(&data[..4]);
        assert_eq!(r, Incomplete(Size(8)));
    }

    #[test]
    fn parse_a_header_with_zero_data() {
        let data = [0, 0, 0, 0, 0, 0, 0, 0];
        let r = super::length_encoded(&data);
        assert_eq!(r, Done(&[][..], &[][..]));
    }

    #[test]
    fn parse_text_encoding() {
        // dd skip=16 count=28 if=rumble.replay of=text.replay bs=1
        let data = include_bytes!("../assets/text.replay");
        let r = super::text_encoded(data);
        assert_eq!(r, Done(&[][..], "TAGame.Replay_Soccar_TA"));
    }

    #[test]
    fn rdict_no_elements() {
        let data = [0x05, 0x00, 0x00, 0x00, b'N', b'o', b'n', b'e', 0x00];
        let r = super::rdict(&data);
        assert_eq!(r, Done(&[][..],  Vec::new()));
    }

    #[test]
    fn rdict_one_element() {
        // dd skip=$((0x1269)) count=$((0x12a8 - 0x1269)) if=rumble.replay of=rdict_one.replay bs=1
        let data = include_bytes!("../assets/rdict_one.replay");
        let r = super::rdict(data);
        assert_eq!(r, Done(&[][..],  vec![("PlayerName".to_string(), super::RProp::Str("comagoosie".to_string()))]));
    }

    #[test]
    fn rdict_one_int_element() {
        // dd skip=$((0x250)) count=$((0x284 - 0x250)) if=rumble.replay of=rdict_int.replay bs=1
        let data = include_bytes!("../assets/rdict_int.replay");
        let r = super::rdict(data);
        assert_eq!(r, Done(&[][..],  vec![("PlayerTeam".to_string(), super::RProp::Int(0))]));
    }

    #[test]
    fn rdict_one_bool_element() {
        // dd skip=$((0xa0f)) count=$((0xa3b - 0xa0f)) if=rumble.replay of=rdict_bool.replay bs=1
        let data = include_bytes!("../assets/rdict_bool.replay");
        let r = super::rdict(data);
        assert_eq!(r, Done(&[][..],  vec![("bBot".to_string(), super::RProp::Bool(false))]));
    }

    fn append_none(input: &[u8]) -> Vec<u8> {
        let append = [0x05, 0x00, 0x00, 0x00, b'N', b'o', b'n', b'e', 0x00];
        let mut v = Vec::new();
        v.extend_from_slice(input);
        v.extend_from_slice(&append);
        v
    }

    #[test]
    fn rdict_one_name_element() {
        // dd skip=$((0x1237)) count=$((0x1269 - 0x1237)) if=rumble.replay of=rdict_name.replay bs=1
        let data = append_none(include_bytes!("../assets/rdict_name.replay"));
        let r = super::rdict(&data);
        assert_eq!(r, Done(&[][..],  vec![("MatchType".to_string(), super::RProp::Name("Online".to_string()))]));

    }

    #[test]
    fn rdict_one_float_element() {
        // dd skip=$((0x10a2)) count=$((0x10ce - 0x10a2)) if=rumble.replay of=rdict_float.replay bs=1
        let data = append_none(include_bytes!("../assets/rdict_float.replay"));
        let r = super::rdict(&data);
        assert_eq!(r, Done(&[][..],  vec![("RecordFPS".to_string(), super::RProp::Float(30.0))]));
    }

    #[test]
    fn rdict_one_qword_element() {
        // dd skip=$((0x576)) count=$((0x5a5 - 0x576)) if=rumble.replay of=rdict_qword.replay bs=1
        let data = append_none(include_bytes!("../assets/rdict_qword.replay"));
        let r = super::rdict(&data);
        assert_eq!(r, Done(&[][..],  vec![("OnlineID".to_string(), super::RProp::QWord(76561198101748375))]));
    }

    #[test]
    fn rdict_one_array_element() {
        // dd skip=$((0x576)) count=$((0x5a5 - 0x576)) if=rumble.replay of=rdict_qword.replay bs=1
        let data = append_none(include_bytes!("../assets/rdict_array.replay"));
        let r = super::rdict(&data);
        let expected = vec![
            vec![
                ("frame".to_string(), Int(441)),
                ("PlayerName".to_string(), Str("Cakeboss".to_string())),
                ("PlayerTeam".to_string(), Int(1))
            ], vec![
                ("frame".to_string(), Int(1738)),
                ("PlayerName".to_string(), Str("Sasha Kaun".to_string())),
                ("PlayerTeam".to_string(), Int(0))
            ], vec![
                ("frame".to_string(), Int(3504)),
                ("PlayerName".to_string(), Str("SilentWarrior".to_string())),
                ("PlayerTeam".to_string(), Int(0))
            ], vec![
                ("frame".to_string(), Int(5058)),
                ("PlayerName".to_string(), Str("jeffreyj1".to_string())),
                ("PlayerTeam".to_string(), Int(1))
            ], vec![
                ("frame".to_string(), Int(5751)),
                ("PlayerName".to_string(), Str("GOOSE LORD".to_string())),
                ("PlayerTeam".to_string(), Int(0))
            ], vec![
                ("frame".to_string(), Int(6083)),
                ("PlayerName".to_string(), Str("GOOSE LORD".to_string())),
                ("PlayerTeam".to_string(), Int(0))
            ], vec![
                ("frame".to_string(), Int(7021)),
                ("PlayerName".to_string(), Str("SilentWarrior".to_string())),
                ("PlayerTeam".to_string(), Int(0))
            ]
        ];
        assert_eq!(r, Done(&[][..],  vec![("Goals".to_string(), super::RProp::Array(expected))]));
    }
}
