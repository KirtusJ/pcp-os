//! The ordered alternatives of the pinned Qwen2 pre-tokenizer; byte offsets in valid UTF-8.
use super::{check,unicode,Error};
fn letter(c:char)->bool{unicode::category(c)==4}
fn number(c:char)->bool{unicode::category(c)==2}
fn space(c:char)->bool{unicode::whitespace(c)}
fn other(c:char)->bool{!space(c)&&!letter(c)&&!number(c)}
pub(super) fn next(text:&str,keep:&mut impl FnMut()->bool)->Result<usize,Error>{
    let first=text.chars().next().ok_or(Error::Utf8)?;
    if first=='\'' {
        for suffix in ["s","t","re","ve","m","ll","d"] {
            if text.as_bytes().get(1..1+suffix.len()).is_some_and(|s|s.eq_ignore_ascii_case(suffix.as_bytes())){return Ok(1+suffix.len());}
        }
    }
    if first!='\r'&&first!='\n'&&!number(first) &&
       (letter(first)||text[first.len_utf8()..].chars().next().is_some_and(letter)) {
        let mut end=first.len_utf8();
        for c in text[end..].chars(){check(keep)?;if !letter(c){break;}end+=c.len_utf8();}return Ok(end);
    }
    if number(first){return Ok(first.len_utf8());}
    let skip=usize::from(first==' ');
    if text[skip..].chars().next().is_some_and(other) {
        let mut end=skip;
        for c in text[skip..].chars(){check(keep)?;if !other(c){break;}end+=c.len_utf8();}
        for c in text[end..].chars(){check(keep)?;if c!='\r'&&c!='\n'{break;}end+=c.len_utf8();}
        return Ok(end);
    }
    let (mut end,mut previous,mut newline)=(0,0,0);
    for c in text.chars(){check(keep)?;if !space(c){break;}previous=end;end+=c.len_utf8();if c=='\r'||c=='\n'{newline=end;}}
    if newline!=0{return Ok(newline);}
    if end<text.len()&&previous!=0{return Ok(previous);}
    if end!=0{return Ok(end);}
    Ok(first.len_utf8())
}
