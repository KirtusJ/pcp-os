//! Bounded GPT-2 byte BPE for the GGUF `gpt2` / `qwen2` tokenizer profile.
//! Vocabulary strings borrow the immutable GGUF prefix. This does not authenticate it.
//! User text is always literal: role/control/added tokens must be inserted by a trusted
//! template through `special_id`, never recognized from user-supplied delimiters.
use alloc::vec::Vec;
use core::{cmp::Ordering, mem::size_of};
use pcpos_gguf::{Array, Index, Value};
#[path="tokenizer_unicode.rs"] mod unicode;
#[path="tokenizer_split.rs"] mod split;

pub const MAX_TEXT_BYTES: usize = 16 * 1024;
pub const MAX_TOKEN_BYTES: usize = 1024;
const MAX_VOCAB: usize = 262_144;
const MAX_MERGES: usize = 262_144;
const NONE: u32 = u32::MAX;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error { Profile, Metadata, Limit, Budget, Allocation, Cancelled, Vocabulary, Merge, Token, Special, Unused, Output, Utf8 }

#[derive(Clone, Copy)]
struct Token<'a> { text: &'a str, kind: u8 }
#[derive(Clone, Copy)]
struct Merge { left: u32, right: u32, rank: u32, result: u32 }
#[derive(Clone, Copy)]
struct Node { token: u32, prev: u32, next: u32 }
#[derive(Clone, Copy)]
struct Pair { rank: u32, left: u32, right: u32, result: u32, a: u32, b: u32 }
impl Pair { fn key(&self)->(u32,u32) { (self.rank,self.left) } }

pub struct Tokenizer<'a> {
    tokens: Vec<Token<'a>>, order: Vec<u32>, merges: Vec<Merge>, bytes: [u32;256],
    bos: u32, eos: u32, padding: u32,
}
impl<'a> Tokenizer<'a> {
    /// Requested heap payload only: excludes allocator overhead, this struct, prefix and stack.
    pub fn table_bytes(index:&Index<'_>)->Result<usize,Error> {
        let (n,m)=counts(index)?;
        n.checked_mul(size_of::<Token>()+size_of::<u32>())
            .and_then(|v|v.checked_add(m.checked_mul(size_of::<Merge>())?)).ok_or(Error::Limit)
    }

    /// Fallible allocations and cancellation, including during in-place table sorting.
    pub fn new(index:&Index<'a>,budget:usize,mut keep:impl FnMut()->bool)->Result<Self,Error> {
        if index.get("tokenizer.ggml.model")!=Some(Value::String("gpt2")) ||
           index.get("tokenizer.ggml.pre")!=Some(Value::String("qwen2")) {return Err(Error::Profile);}
        for (k,expected) in [("tokenizer.ggml.add_bos_token",false),("tokenizer.ggml.add_eos_token",false),
                             ("tokenizer.ggml.add_space_prefix",false),("tokenizer.ggml.remove_extra_whitespaces",false)] {
            if let Some(v)=index.get(k) {if v!=Value::Bool(expected){return Err(Error::Profile);}}
        }
        // Do not silently reinterpret another tokenizer's settings under this profile name.
        for item in index.metadata() {
            if item.key.starts_with("tokenizer.") && !matches!(item.key,
                "tokenizer.ggml.model"|"tokenizer.ggml.pre"|"tokenizer.ggml.tokens"|"tokenizer.ggml.token_type"|
                "tokenizer.ggml.merges"|"tokenizer.ggml.bos_token_id"|"tokenizer.ggml.eos_token_id"|
                "tokenizer.ggml.padding_token_id"|"tokenizer.ggml.add_bos_token"|"tokenizer.ggml.add_eos_token"|
                "tokenizer.ggml.add_space_prefix"|"tokenizer.ggml.remove_extra_whitespaces"|"tokenizer.chat_template") {
                return Err(Error::Profile);
            }
        }
        let (n,m)=counts(index)?;
        if Self::table_bytes(index)?>budget{return Err(Error::Budget);}
        check(&mut keep)?;
        let mut tokens=reserve(n)?;let mut order=reserve(n)?;let mut merges=reserve(m)?;
        let kinds=array(index,"tokenizer.ggml.token_type",5)?;
        if kinds.len()!=n as u64{return Err(Error::Metadata);}
        for (i,(text,kind)) in array(index,"tokenizer.ggml.tokens",8)?.iter().zip(kinds.iter()).enumerate() {
            check(&mut keep)?;
            let text=match text {Ok(Value::String(s))=>s,_=>return Err(Error::Metadata)};
            let kind=match kind {Ok(Value::I32(v @ (1|3|4|5)))=>v as u8,_=>return Err(Error::Vocabulary)};
            if text.is_empty()||text.len()>MAX_TOKEN_BYTES{return Err(Error::Vocabulary);}
            if kind==1 && text.chars().any(|c|byte_of(c).is_none()){return Err(Error::Vocabulary);}
            tokens.push(Token{text,kind});order.push(i as u32);
        }
        sort(&mut order,|a,b|tokens[*a as usize].text.cmp(tokens[*b as usize].text),&mut keep)?;
        for ids in order.windows(2) {check(&mut keep)?;if tokens[ids[0] as usize].text==tokens[ids[1] as usize].text{return Err(Error::Vocabulary);}}
        let id=|key:&str|->Result<u32,Error>{match index.get(key) {
            Some(Value::U32(v)) if (v as usize)<n && tokens[v as usize].kind==3=>Ok(v),_=>Err(Error::Metadata)}};
        let (bos,eos,padding)=(id("tokenizer.ggml.bos_token_id")?,id("tokenizer.ggml.eos_token_id")?,id("tokenizer.ggml.padding_token_id")?);
        let mut result=Self{tokens,order,merges:Vec::new(),bytes:[NONE;256],bos,eos,padding};
        for b in 0..256 {
            check(&mut keep)?;let mut utf8=[0;4];let s=char_of(b as u8).encode_utf8(&mut utf8);
            let token=result.lookup(s).ok_or(Error::Vocabulary)?;
            if result.tokens[token as usize].kind!=1{return Err(Error::Vocabulary);}
            result.bytes[b]=token;
        }
        let mut joined=[0;MAX_TOKEN_BYTES];
        for (rank,value) in array(index,"tokenizer.ggml.merges",8)?.iter().enumerate() {
            check(&mut keep)?;
            let text=match value {Ok(Value::String(s))=>s,_=>return Err(Error::Metadata)};
            let (a,b)=text.split_once(' ').ok_or(Error::Merge)?;
            let length=a.len().checked_add(b.len()).ok_or(Error::Merge)?;
            if a.is_empty()||b.is_empty()||length>MAX_TOKEN_BYTES{return Err(Error::Merge);}
            let left=result.lookup(a).ok_or(Error::Merge)?;let right=result.lookup(b).ok_or(Error::Merge)?;
            joined[..a.len()].copy_from_slice(a.as_bytes());joined[a.len()..length].copy_from_slice(b.as_bytes());
            let text=core::str::from_utf8(&joined[..length]).map_err(|_|Error::Merge)?;
            let target=result.lookup(text).ok_or(Error::Merge)?;
            if [left,right,target].iter().any(|i|result.tokens[*i as usize].kind!=1){return Err(Error::Merge);}
            merges.push(Merge{left,right,rank:rank as u32,result:target});
        }
        sort(&mut merges,|a,b|(a.left,a.right).cmp(&(b.left,b.right)),&mut keep)?;
        for p in merges.windows(2){check(&mut keep)?;if (p[0].left,p[0].right)==(p[1].left,p[1].right){return Err(Error::Merge);}}
        result.merges=merges;Ok(result)
    }
    pub fn vocabulary(&self)->usize {self.tokens.len()}
    pub fn bos(&self)->u32 {self.bos}
    pub fn eos(&self)->u32 {self.eos}
    pub fn padding(&self)->u32 {self.padding}
    /// Normal and user-defined text tokens; excludes controls and unused padding entries.
    pub fn is_text(&self,id:u32)->bool {self.tokens.get(id as usize).is_some_and(|t|matches!(t.kind,1|4))}
    /// Exact added/control name for a trusted template; not a parser for user text.
    pub fn special_id(&self,text:&str)->Option<u32> {
        self.lookup(text).filter(|i|matches!(self.tokens[*i as usize].kind,3|4))
    }
    fn lookup(&self,text:&str)->Option<u32> {
        self.order.binary_search_by(|i|self.tokens[*i as usize].text.cmp(text)).ok().map(|i|self.order[i])
    }
    fn pair(&self,left:u32,right:u32,nodes:&[Node])->Option<Pair> {
        if left==NONE||right==NONE{return None;}
        let a=nodes[left as usize].token;let b=nodes[right as usize].token;
        let i=self.merges.binary_search_by_key(&(a,b),|m|(m.left,m.right)).ok()?;
        let m=self.merges[i];Some(Pair{rank:m.rank,left,right,result:m.result,a,b})
    }
    /// Requested scratch plus returned-token payload; excludes stack and allocator overhead.
    pub fn encode_bytes(text_bytes:usize,max_tokens:usize)->Result<usize,Error> {
        if text_bytes>MAX_TEXT_BYTES||max_tokens>MAX_TEXT_BYTES{return Err(Error::Limit);}
        Ok(text_bytes*(size_of::<Node>()+3*size_of::<Pair>())+text_bytes.min(max_tokens)*size_of::<u32>())
    }
    /// No normalization, BOS/EOS insertion, or delimiter recognition. Errors return no partial tokens.
    pub fn encode(&self,text:&str,max_tokens:usize,budget:usize,mut keep:impl FnMut()->bool)->Result<Vec<u32>,Error> {
        if Self::encode_bytes(text.len(),max_tokens)?>budget{return Err(Error::Budget);}
        check(&mut keep)?;
        let mut output=reserve(text.len().min(max_tokens))?;
        let mut nodes=reserve(text.len())?;let mut heap=reserve(text.len()*3)?;
        let mut at=0;
        while at<text.len() {
            check(&mut keep)?;
            let length=split::next(&text[at..],&mut keep)?;
            nodes.clear();heap.clear();
            for (i,b) in text.as_bytes()[at..at+length].iter().enumerate() {
                check(&mut keep)?;
                nodes.push(Node{token:self.bytes[*b as usize],prev:if i==0{NONE}else{(i-1) as u32},next:if i+1==length{NONE}else{(i+1) as u32}});
            }
            for i in 1..nodes.len(){check(&mut keep)?;if let Some(p)=self.pair((i-1) as u32,i as u32,&nodes){push(&mut heap,p);}}
            while let Some(p)=pop(&mut heap) {
                check(&mut keep)?;
                let left=p.left as usize;let right=p.right as usize;
                if nodes[left].token!=p.a||nodes[right].token!=p.b||nodes[left].next!=p.right{continue;}
                nodes[left].token=p.result;nodes[left].next=nodes[right].next;nodes[right].token=NONE;
                if nodes[left].next!=NONE{let next=nodes[left].next as usize;nodes[next].prev=p.left;}
                for (a,b) in [(nodes[left].prev,p.left),(p.left,nodes[left].next)] {
                    if let Some(pair)=self.pair(a,b,&nodes){push(&mut heap,pair);}
                }
            }
            for node in &nodes {
                check(&mut keep)?;
                if node.token!=NONE {
                    if output.len()==max_tokens{return Err(Error::Output);}
                    output.push(node.token);
                }
            }
            at+=length;
        }
        Ok(output)
    }
    /// Decode one normal or user-defined token to raw bytes, which may end inside UTF-8.
    /// Control tokens and unused vocabulary entries cannot leak into display text.
    pub fn piece(&self,id:u32,output:&mut [u8])->Result<usize,Error> {
        let t=self.tokens.get(id as usize).ok_or(Error::Token)?;
        if t.kind==3{return Err(Error::Special);}if t.kind==5{return Err(Error::Unused);}
        let n=if t.kind==4{t.text.len()}else{t.text.chars().count()};
        if output.len()<n{return Err(Error::Output);}
        if t.kind==4{output[..n].copy_from_slice(t.text.as_bytes());}
        else{for (i,c) in t.text.chars().enumerate(){output[i]=byte_of(c).ok_or(Error::Vocabulary)?;}}
        Ok(n)
    }
}

/// Holds at most three unfinished UTF-8 bytes between token pieces. Invalid UTF-8 is an
/// explicit failure; the caller can terminate that reply instead of silently replacing bytes.
pub struct Utf8Stream { pending:[u8;3], length:usize }
impl Default for Utf8Stream {fn default()->Self{Self::new()}}
impl Utf8Stream {
    pub const fn new()->Self{Self{pending:[0;3],length:0}}
    pub fn clear(&mut self){self.pending=[0;3];self.length=0;}
    pub fn finish(&self)->Result<(),Error>{if self.length==0{Ok(())}else{Err(Error::Utf8)}}
    pub fn push(&mut self,piece:&[u8],mut emit:impl FnMut(&str))->Result<(),Error>{
        if piece.len()>MAX_TOKEN_BYTES{return Err(Error::Limit);}
        let mut buffer=[0;MAX_TOKEN_BYTES+3];buffer[..self.length].copy_from_slice(&self.pending[..self.length]);
        buffer[self.length..self.length+piece.len()].copy_from_slice(piece);let n=self.length+piece.len();
        let valid=match core::str::from_utf8(&buffer[..n]) {Ok(_)=>n,Err(e) if e.error_len().is_none()=>e.valid_up_to(),Err(_)=>return Err(Error::Utf8)};
        let tail=n-valid;if tail>3{return Err(Error::Utf8);}
        self.pending[..tail].copy_from_slice(&buffer[valid..n]);self.length=tail;
        if valid!=0{emit(core::str::from_utf8(&buffer[..valid]).map_err(|_|Error::Utf8)?);}
        Ok(())
    }
}

fn array<'a>(i:&Index<'a>,key:&str,kind:u32)->Result<Array<'a>,Error>{match i.get(key){Some(Value::Array(a)) if a.value_type()==kind=>Ok(a),_=>Err(Error::Metadata)}}
fn counts(i:&Index<'_>)->Result<(usize,usize),Error>{
    let n=array(i,"tokenizer.ggml.tokens",8)?.len();let m=array(i,"tokenizer.ggml.merges",8)?.len();
    if !(256..=MAX_VOCAB as u64).contains(&n)||m>MAX_MERGES as u64{return Err(Error::Limit);}Ok((n as usize,m as usize))
}
fn check(keep:&mut impl FnMut()->bool)->Result<(),Error>{if keep(){Ok(())}else{Err(Error::Cancelled)}}
fn reserve<T>(n:usize)->Result<Vec<T>,Error>{let mut v=Vec::new();v.try_reserve_exact(n).map_err(|_|Error::Allocation)?;Ok(v)}

// GPT-2's reversible byte alphabet: printable Latin-1 bytes keep their codepoint;
// the remaining bytes are assigned U+0100 upwards, in byte order.
fn direct(b:u32)->bool{(33..=126).contains(&b)||(161..=172).contains(&b)||(174..=255).contains(&b)}
fn char_of(b:u8)->char {
    let b=b as u32;
    let c=if direct(b){b}else{256+(0..b).filter(|n|!direct(*n)).count() as u32};
    char::from_u32(c).unwrap()
}
fn byte_of(c:char)->Option<u8>{
    let c=c as u32;if direct(c){return Some(c as u8);}
    if !(256..=323).contains(&c){return None;}
    (0u32..=255).filter(|b|!direct(*b)).nth((c-256) as usize).map(|b|b as u8)
}

fn sort<T>(items:&mut [T],compare:impl Fn(&T,&T)->Ordering,keep:&mut impl FnMut()->bool)->Result<(),Error>{
    fn down<T>(a:&mut [T],mut root:usize,end:usize,cmp:&impl Fn(&T,&T)->Ordering,keep:&mut impl FnMut()->bool)->Result<(),Error>{
        loop{check(keep)?;let mut child=root*2+1;if child>=end{return Ok(());}
            if child+1<end&&cmp(&a[child],&a[child+1])==Ordering::Less{child+=1;}
            if cmp(&a[root],&a[child])!=Ordering::Less{return Ok(());}a.swap(root,child);root=child;}
    }
    for root in (0..items.len()/2).rev(){down(items,root,items.len(),&compare,keep)?;}
    for end in (1..items.len()).rev(){check(keep)?;items.swap(0,end);down(items,0,end,&compare,keep)?;}Ok(())
}
fn push(heap:&mut Vec<Pair>,pair:Pair){
    heap.push(pair);let mut i=heap.len()-1;
    while i>0{let parent=(i-1)/2;if heap[parent].key()<=heap[i].key(){break;}heap.swap(parent,i);i=parent;}
}
fn pop(heap:&mut Vec<Pair>)->Option<Pair>{
    let last=heap.pop()?;if heap.is_empty(){return Some(last);}
    let result=core::mem::replace(&mut heap[0],last);let mut root=0;
    loop{let mut child=root*2+1;if child>=heap.len(){break;}
        if child+1<heap.len()&&heap[child+1].key()<heap[child].key(){child+=1;}
        if heap[root].key()<=heap[child].key(){break;}heap.swap(root,child);root=child;}
    Some(result)
}
